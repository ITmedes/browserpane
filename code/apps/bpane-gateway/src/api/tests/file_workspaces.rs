use super::*;

#[tokio::test]
async fn creates_lists_uploads_downloads_and_deletes_file_workspace_content() {
    let (app, token) = test_router();

    let create_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/file-workspaces")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "finance-reports",
                        "description": "Shared workflow outputs",
                        "labels": {
                            "suite": "contract"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workspace_response.status(), StatusCode::CREATED);
    let workspace = response_json(create_workspace_response).await;
    let workspace_id = workspace["id"].as_str().unwrap().to_string();
    assert_eq!(workspace["name"], "finance-reports");
    assert_eq!(workspace["description"], "Shared workflow outputs");
    assert_eq!(workspace["labels"]["suite"], "contract");
    assert_eq!(
        workspace["files_path"],
        format!("/api/v1/file-workspaces/{workspace_id}/files")
    );

    let list_workspaces_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/file-workspaces")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_workspaces_response.status(), StatusCode::OK);
    let workspaces = response_json(list_workspaces_response).await;
    assert_eq!(workspaces["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(workspaces["workspaces"][0]["id"], workspace_id);

    let get_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_workspace_response.status(), StatusCode::OK);
    let fetched_workspace = response_json(get_workspace_response).await;
    assert_eq!(fetched_workspace["id"], workspace_id);

    let file_bytes = b"alpha,beta\n1,2\n";
    let file_hash = hex::encode(Sha256::digest(file_bytes));
    let provenance = json!({
        "source_kind": "git_materialized",
        "repo_path": "workflows/exports/report.csv",
        "commit": "abc123def456"
    });
    let upload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .header("content-type", "text/csv")
                .header("x-bpane-file-name", "report.csv")
                .header("x-bpane-file-provenance", provenance.to_string())
                .body(Body::from(file_bytes.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload_response.status(), StatusCode::CREATED);
    let uploaded = response_json(upload_response).await;
    let file_id = uploaded["id"].as_str().unwrap().to_string();
    assert_eq!(uploaded["workspace_id"], workspace_id);
    assert_eq!(uploaded["name"], "report.csv");
    assert_eq!(uploaded["media_type"], "text/csv");
    assert_eq!(uploaded["byte_count"], file_bytes.len());
    assert_eq!(uploaded["sha256_hex"], file_hash);
    assert_eq!(uploaded["provenance"], provenance);
    assert_eq!(
        uploaded["content_path"],
        format!("/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content")
    );

    let list_files_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_files_response.status(), StatusCode::OK);
    let files = response_json(list_files_response).await;
    assert_eq!(files["files"].as_array().unwrap().len(), 1);
    assert_eq!(files["files"][0]["id"], file_id);

    let get_file_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/file-workspaces/{workspace_id}/files/{file_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_file_response.status(), StatusCode::OK);
    let fetched_file = response_json(get_file_response).await;
    assert_eq!(fetched_file["id"], file_id);
    assert_eq!(fetched_file["sha256_hex"], file_hash);

    let content_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content_response.status(), StatusCode::OK);
    assert_eq!(
        content_response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/csv"
    );
    let downloaded = to_bytes(content_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(downloaded.as_ref(), file_bytes);

    let delete_file_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/v1/file-workspaces/{workspace_id}/files/{file_id}"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_file_response.status(), StatusCode::OK);
    let deleted = response_json(delete_file_response).await;
    assert_eq!(deleted["id"], file_id);

    let final_list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/file-workspaces/{workspace_id}/files"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_list_response.status(), StatusCode::OK);
    let final_files = response_json(final_list_response).await;
    assert!(final_files["files"].as_array().unwrap().is_empty());
}
