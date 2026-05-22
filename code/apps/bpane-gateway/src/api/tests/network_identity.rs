use super::*;

#[tokio::test]
async fn manages_egress_profiles_and_session_network_identity() {
    let (app, token) = test_router_with_docker_pool().await;

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/egress-profiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let disabled_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/egress-profiles")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "disabled-egress",
                        "state": "disabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled_profile_response.status(), StatusCode::CREATED);
    let disabled_profile = response_json(disabled_profile_response).await;
    let disabled_profile_id = disabled_profile["id"].as_str().unwrap().to_string();

    let disabled_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "network_identity": {
                            "egress_profile_id": disabled_profile_id
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled_session_response.status(), StatusCode::CONFLICT);

    let invalid_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/egress-profiles")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bad-proxy",
                        "proxy": { "url": "https://user:pass@proxy.example:8443" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_profile_response.status(), StatusCode::BAD_REQUEST);

    let invalid_tls_observation_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/egress-profiles")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bad-tls-observer",
                        "proxy": { "url": "https://proxy.example:8443" },
                        "custom_ca": { "certificate_ref": "file:///workspace/dev/egress-ca.pem" },
                        "traffic_observation": { "mode": "tls_intercept" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        invalid_tls_observation_response.status(),
        StatusCode::BAD_REQUEST
    );

    let missing_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "network_identity": {
                            "egress_profile_id": Uuid::now_v7()
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_profile_response.status(), StatusCode::NOT_FOUND);

    let invalid_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "network_identity": {
                            "locale": "bad locale",
                            "geolocation": {
                                "latitude": 91.0,
                                "longitude": 13.4
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_session_response.status(), StatusCode::BAD_REQUEST);

    let create_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/egress-profiles")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "eu-support-egress",
                        "description": "Support EU egress",
                        "labels": { "region": "eu" },
                        "proxy": { "url": "https://proxy.example:8443" },
                        "bypass_rules": ["localhost", "*.internal.example"],
                        "custom_ca": {
                            "certificate_ref": "file:///workspace/dev/egress-ca.pem",
                            "display_name": "EU support CA"
                        },
                        "traffic_observation": {
                            "mode": "tls_intercept",
                            "sensitive_log_sink_ref": "siem://browserpane/eu-support",
                            "sensitive_log_sink_display_name": "EU support SIEM"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_profile_response.status(), StatusCode::CREATED);
    let profile = response_json(create_profile_response).await;
    let profile_id = profile["id"].as_str().unwrap().to_string();
    assert_eq!(profile["name"], "eu-support-egress");
    assert_eq!(profile["state"], "ready");
    assert_eq!(profile["effective"]["proxy_configured"], true);
    assert_eq!(profile["effective"]["bypass_rule_count"], 2);
    assert_eq!(profile["effective"]["custom_ca_configured"], true);
    assert_eq!(profile["effective"]["observation_mode"], "tls_intercept");
    assert_eq!(profile["effective"]["tls_interception_enabled"], true);
    assert_eq!(profile["effective"]["sensitive_log_sink_configured"], true);
    assert_eq!(
        profile["traffic_observation"]["sensitive_log_sink_ref"],
        "siem://browserpane/eu-support"
    );
    assert_eq!(profile["diagnostics"]["health"], "ready");
    assert_eq!(profile["diagnostics"]["proof_level"], "configuration");
    assert_eq!(
        profile["diagnostics"]["proof"]["active_probe_collected"],
        false
    );
    assert_eq!(
        profile["diagnostics"]["proof"]["tls_interception_expected"],
        true
    );

    let get_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/egress-profiles/{profile_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_profile_response.status(), StatusCode::OK);
    let fetched_profile = response_json(get_profile_response).await;
    assert_eq!(fetched_profile["id"], profile_id);

    let profile_diagnostics_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/egress-profiles/{profile_id}/diagnostics"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(profile_diagnostics_response.status(), StatusCode::OK);
    let profile_diagnostics = response_json(profile_diagnostics_response).await;
    assert_eq!(profile_diagnostics["profile_id"], profile_id);
    assert_eq!(profile_diagnostics["health"], "ready");
    assert_eq!(profile_diagnostics["runtime_binding"], Value::Null);
    assert_eq!(
        profile_diagnostics["proof"]["sensitive_log_sink_declared"],
        true
    );

    let update_profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/egress-profiles/{profile_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "eu-support-egress-v2",
                        "description": "Updated support EU egress",
                        "labels": { "region": "eu", "managed": "true" },
                        "proxy": { "url": "https://proxy.example:8443" },
                        "bypass_rules": ["localhost", "*.internal.example"],
                        "custom_ca": {
                            "certificate_ref": "file:///workspace/dev/egress-ca.pem",
                            "display_name": "EU support CA"
                        },
                        "traffic_observation": {
                            "mode": "tls_intercept",
                            "sensitive_log_sink_ref": "siem://browserpane/eu-support",
                            "sensitive_log_sink_display_name": "EU support SIEM"
                        },
                        "state": "ready"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_profile_response.status(), StatusCode::OK);
    let updated_profile = response_json(update_profile_response).await;
    assert_eq!(updated_profile["id"], profile_id);
    assert_eq!(updated_profile["name"], "eu-support-egress-v2");
    assert_eq!(updated_profile["labels"]["managed"], "true");
    assert_eq!(
        updated_profile["effective"]["tls_interception_enabled"],
        true
    );

    let missing_update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/egress-profiles/{}", Uuid::now_v7()))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "missing-egress"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_update_response.status(), StatusCode::NOT_FOUND);

    let invalid_update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/egress-profiles/{profile_id}"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "bad-update",
                        "proxy": { "url": "http://user:pass@proxy.example:3128" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_update_response.status(), StatusCode::BAD_REQUEST);

    let create_template_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/session-templates")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "customer-eu-debug",
                        "defaults": {
                            "network_identity": {
                                "locale": "de-DE",
                                "languages": ["de-DE", "en-US"],
                                "timezone": "Europe/Berlin",
                                "geolocation": {
                                    "latitude": 52.52,
                                    "longitude": 13.405,
                                    "accuracy_meters": 100.0
                                },
                                "browser_identity": "desktop-chromium-stable",
                                "egress_profile_id": profile_id
                            },
                            "labels": { "region": "eu" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_template_response.status(), StatusCode::CREATED);
    let template = response_json(create_template_response).await;
    let template_id = template["id"].as_str().unwrap().to_string();

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template_id": template_id,
                        "network_identity": {
                            "timezone": "UTC",
                            "user_agent": "BrowserPaneTest/1.0"
                        },
                        "labels": { "case": "INC-1234" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();
    assert_eq!(session["network_identity"]["locale"], "de-DE");
    assert_eq!(session["network_identity"]["languages"][0], "de-DE");
    assert_eq!(session["network_identity"]["timezone"], "UTC");
    assert_eq!(
        session["network_identity"]["user_agent"],
        "BrowserPaneTest/1.0"
    );
    assert_eq!(
        session["network_identity"]["browser_identity"],
        "desktop-chromium-stable"
    );
    assert_eq!(session["network_identity"]["egress_profile_id"], profile_id);
    assert_eq!(session["effective_egress"]["profile_id"], profile_id);
    assert_eq!(
        session["effective_egress"]["profile_name"],
        "eu-support-egress-v2"
    );
    assert_eq!(session["effective_egress"]["bypass_rule_count"], 2);
    assert_eq!(
        session["effective_egress"]["observation_mode"],
        "tls_intercept"
    );
    assert_eq!(
        session["effective_egress"]["tls_interception_enabled"],
        true
    );
    assert_eq!(session["egress_diagnostics"]["profile_id"], profile_id);
    assert_eq!(session["egress_diagnostics"]["health"], "unknown");
    assert_eq!(
        session["egress_diagnostics"]["runtime_binding"],
        "docker_runtime_pool"
    );
    assert_eq!(
        session["egress_diagnostics"]["proof"]["runtime_launch_observed"],
        false
    );
    assert!(session["egress_diagnostics"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning
            .as_str()
            .unwrap()
            .contains("No active runtime launch metadata")));
    assert_eq!(session["labels"]["region"], "eu");
    assert_eq!(session["labels"]["case"], "INC-1234");

    let status_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
    let status = response_json(status_response).await;
    assert_eq!(status["network_identity"]["timezone"], "UTC");
    assert_eq!(
        status["effective_egress"]["profile_name"],
        "eu-support-egress-v2"
    );
    assert_eq!(status["effective_egress"]["tls_interception_enabled"], true);
    assert_eq!(status["egress_diagnostics"]["profile_id"], profile_id);
    assert_eq!(status["egress_diagnostics"]["health"], "unknown");

    let session_diagnostics_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/egress-diagnostics"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(session_diagnostics_response.status(), StatusCode::OK);
    let session_diagnostics = response_json(session_diagnostics_response).await;
    assert_eq!(session_diagnostics["profile_name"], "eu-support-egress-v2");
    assert_eq!(
        session_diagnostics["proof"]["custom_ca_launch_config_expected"],
        true
    );

    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/egress-profiles")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list = response_json(list_response).await;
    assert_eq!(list["profiles"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn active_egress_probe_failures_are_persisted_as_session_diagnostics() {
    let (app, token) = test_router();

    let create_session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_session_response.status(), StatusCode::CREATED);
    let session = response_json(create_session_response).await;
    let session_id = session["id"].as_str().unwrap().to_string();

    let invalid_probe_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/egress-diagnostics"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tls_probe_url": "http://example.com"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_probe_response.status(), StatusCode::BAD_REQUEST);

    let probe_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/egress-diagnostics"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "public_ip_url": "https://example.com/",
                        "tls_probe_url": "https://example.com/",
                        "timeout_ms": 250
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(probe_response.status(), StatusCode::OK);
    let probe = response_json(probe_response).await;
    assert_eq!(probe["health"], "attention");
    assert_eq!(probe["proof"]["active_probe_collected"], false);
    assert!(probe["proof"]["last_failure_reason"].as_str().is_some());
    assert!(probe["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning
            .as_str()
            .unwrap()
            .contains("Last active egress probe failed")));

    let fetched_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/egress-diagnostics"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched_response.status(), StatusCode::OK);
    let fetched = response_json(fetched_response).await;
    assert_eq!(
        fetched["proof"]["last_failure_reason"],
        probe["proof"]["last_failure_reason"]
    );
}
