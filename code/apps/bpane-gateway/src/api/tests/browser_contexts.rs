use std::io::Read;

use super::*;

#[tokio::test]
async fn manages_browser_context_catalog_and_reusable_session_binding() {
    let (app, token) = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "support-profile",
                        "description": "Support engineer profile",
                        "labels": { "team": "support" },
                        "retention_sec": 86400,
                        "max_profile_storage_bytes": 1048576
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let context = response_json(create_response).await;
    let context_id = context["id"].as_str().unwrap().to_string();
    assert_eq!(context["name"], "support-profile");
    assert_eq!(context["persistence_mode"], "reusable");
    assert_eq!(context["retention_sec"], 86400);
    assert!(!context["retention_expires_at"].is_null());
    assert_eq!(context["max_profile_storage_bytes"], 1048576);
    assert_eq!(context["state"], "ready");
    assert_eq!(context["usage"]["visible_session_count"], 0);
    assert_eq!(context["usage"]["active_runtime_session_count"], 0);
    assert!(context["usage"]["active_runtime_session_id"].is_null());
    assert!(context["usage"]["profile_storage_bytes"].is_null());
    assert_eq!(context["usage"]["profile_storage_limit_exceeded"], false);
    assert!(context["project_id"].is_null());
    assert!(context["project"].is_null());
    assert!(context["last_used_at"].is_null());

    let duplicate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "support-profile" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_response.status(), StatusCode::CONFLICT);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/browser-contexts/{context_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched = response_json(get_response).await;
    assert_eq!(fetched["id"], context_id);
    assert_eq!(fetched["usage"]["visible_session_count"], 0);
    assert!(fetched["usage"]["profile_storage_bytes"].is_null());

    let export_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/browser-contexts/{context_id}/export"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_response.status(), StatusCode::OK);
    assert_eq!(
        export_response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/zip"
    );
    let export_bytes = response_bytes(export_response).await;
    let cursor = Cursor::new(export_bytes.clone());
    let mut archive = ZipArchive::new(cursor).unwrap();
    let mut manifest_file = archive.by_name("manifest.json").unwrap();
    let mut manifest_bytes = Vec::new();
    manifest_file.read_to_end(&mut manifest_bytes).unwrap();
    drop(manifest_file);
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["format_version"], 1);
    assert_eq!(manifest["archive_type"], "browser_context_export");
    assert_eq!(manifest["source_context"]["id"], context_id);
    assert!(manifest["profile_archive_path"].is_null());
    assert!(archive.by_name("profile.tar.gz").is_err());

    let import_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts/import")
                .header("authorization", bearer(&token))
                .header("content-type", "application/zip")
                .header("x-bpane-browser-context-name", "support-profile-import")
                .header(
                    "x-bpane-browser-context-labels",
                    json!({ "imported": "true" }).to_string(),
                )
                .header("x-bpane-browser-context-retention-sec", "43200")
                .body(Body::from(export_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_response.status(), StatusCode::CREATED);
    let imported_context = response_json(import_response).await;
    let imported_context_id = imported_context["id"].as_str().unwrap().to_string();
    assert_ne!(imported_context_id, context_id);
    assert_eq!(imported_context["name"], "support-profile-import");
    assert_eq!(imported_context["description"], "Support engineer profile");
    assert_eq!(imported_context["labels"]["imported"], "true");
    assert_eq!(imported_context["persistence_mode"], "reusable");
    assert_eq!(imported_context["retention_sec"], 43200);
    assert_eq!(imported_context["max_profile_storage_bytes"], 1048576);
    assert_eq!(imported_context["usage"]["visible_session_count"], 0);

    let clone_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/browser-contexts/{context_id}/clone"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "support-profile-sandbox",
                        "description": "Sandbox copy",
                        "labels": { "team": "support", "copy": "sandbox" },
                        "retention_sec": 43200,
                        "max_profile_storage_bytes": 2097152
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clone_response.status(), StatusCode::CREATED);
    let cloned_context = response_json(clone_response).await;
    let cloned_context_id = cloned_context["id"].as_str().unwrap().to_string();
    assert_ne!(cloned_context_id, context_id);
    assert_eq!(cloned_context["name"], "support-profile-sandbox");
    assert_eq!(cloned_context["description"], "Sandbox copy");
    assert_eq!(cloned_context["labels"]["copy"], "sandbox");
    assert_eq!(cloned_context["persistence_mode"], "reusable");
    assert_eq!(cloned_context["retention_sec"], 43200);
    assert_eq!(cloned_context["max_profile_storage_bytes"], 2097152);
    assert_eq!(cloned_context["usage"]["visible_session_count"], 0);

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(session_response.status(), StatusCode::CREATED);
    let session = response_json(session_response).await;
    assert_eq!(session["browser_context"]["mode"], "reusable");
    assert_eq!(session["browser_context"]["context_id"], context_id);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list = response_json(list_response).await;
    let contexts = list["contexts"].as_array().unwrap();
    assert_eq!(contexts.len(), 3);
    let source_context = contexts
        .iter()
        .find(|context| context["id"] == context_id)
        .unwrap();
    assert_eq!(source_context["usage"]["visible_session_count"], 1);
    assert_eq!(source_context["usage"]["active_runtime_session_count"], 0);
    assert!(source_context["usage"]["active_runtime_session_id"].is_null());
    assert!(source_context["usage"]["profile_storage_bytes"].is_null());
    assert!(!source_context["last_used_at"].is_null());

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/browser-contexts/{context_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted = response_json(delete_response).await;
    assert_eq!(deleted["state"], "deleted");
    assert_eq!(deleted["usage"]["visible_session_count"], 1);
    assert!(!deleted["deleted_at"].is_null());

    let deleted_clone_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/browser-contexts/{context_id}/clone"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "deleted-copy" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted_clone_response.status(), StatusCode::CONFLICT);

    let deleted_export_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/browser-contexts/{context_id}/export"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted_export_response.status(), StatusCode::CONFLICT);

    let deleted_context_session = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted_context_session.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn scopes_browser_contexts_to_projects_for_session_reuse() {
    let (app, token) = test_router();

    let project_a = create_project(&app, &token, "tenant-a").await;
    let project_a_id = project_a["id"].as_str().unwrap().to_string();
    let project_b = create_project(&app, &token, "tenant-b").await;
    let project_b_id = project_b["id"].as_str().unwrap().to_string();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_a_id,
                        "name": "tenant-a-profile"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let context = response_json(create_response).await;
    let context_id = context["id"].as_str().unwrap().to_string();
    assert_eq!(context["project_id"], project_a_id);
    assert_eq!(context["project"]["id"], project_a_id);
    assert_eq!(context["project"]["name"], "tenant-a");

    let mismatched_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_b_id,
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatched_session.status(), StatusCode::BAD_REQUEST);
    let rejected = response_json(mismatched_session).await;
    assert!(rejected["error"]
        .as_str()
        .unwrap()
        .contains("requires a matching session project_id"));

    let matched_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_a_id,
                        "browser_context": {
                            "mode": "reusable",
                            "context_id": context_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(matched_session.status(), StatusCode::CREATED);

    let clone_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/browser-contexts/{context_id}/clone"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "name": "tenant-a-profile-copy" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clone_response.status(), StatusCode::CREATED);
    let cloned = response_json(clone_response).await;
    assert_eq!(cloned["project_id"], project_a_id);
    assert_eq!(cloned["project"]["id"], project_a_id);
}

async fn create_project(app: &Router, token: &str, name: &str) -> Value {
    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header("authorization", bearer(token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "name": name }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn rejects_invalid_browser_context_requests_and_bindings() {
    let (app, token) = test_router();

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/browser-contexts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    for body in [
        json!({ "name": "" }),
        json!({ "name": "bad-description", "description": "" }),
        json!({ "name": "bad-label", "labels": { "": "value" } }),
        json!({ "name": "bad-label-value", "labels": { "team": "" } }),
        json!({ "name": "bad-retention", "retention_sec": 0 }),
        json!({ "name": "bad-storage-limit", "max_profile_storage_bytes": 0 }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/browser-contexts")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let missing_context_id = Uuid::now_v7();
    let missing_get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/browser-contexts/{missing_context_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_get.status(), StatusCode::NOT_FOUND);

    let missing_clone = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/browser-contexts/{missing_context_id}/clone"
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "copy" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_clone.status(), StatusCode::NOT_FOUND);

    let invalid_import = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/browser-contexts/import")
                .header("authorization", bearer(&token))
                .header("content-type", "application/zip")
                .header("x-bpane-browser-context-name", "bad-import")
                .body(Body::from("not a zip archive"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_import.status(), StatusCode::BAD_REQUEST);

    for body in [
        json!({ "browser_context": { "mode": "reusable" } }),
        json!({
            "browser_context": {
                "mode": "fresh",
                "context_id": Uuid::now_v7().to_string()
            }
        }),
        json!({
            "browser_context": {
                "mode": "reusable",
                "context_id": missing_context_id.to_string()
            }
        }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
            ),
            "unexpected status: {}",
            response.status()
        );
    }
}
