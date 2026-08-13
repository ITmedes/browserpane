use bpane_runtime_contract::{
    BrokerApiVersion, BrowserEgressObservationMode, BrowserEgressSelection, BrowserNetworkIdentity,
    BrowserProxySelection, BrowserRuntimeFeatures, BrowserRuntimeLaunchRequest,
    BrowserSessionDataSource, IdempotencyKey, RuntimeOperation, RuntimeOperationRequest,
    RuntimeOperationResult, WorkerExecutionState,
};
use uuid::Uuid;

#[test]
fn browser_launch_request_has_stable_v1_json_shape() {
    let request = RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf00").unwrap(),
        idempotency_key: IdempotencyKey::new("browser:launch:019db438").unwrap(),
        operation: RuntimeOperation::LaunchBrowser(BrowserRuntimeLaunchRequest {
            session_id: Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap(),
            browser_context_id: None,
            features: Default::default(),
        }),
    };

    let json = serde_json::to_string_pretty(&request).unwrap();

    assert_eq!(
        json,
        r#"{
  "api_version": "v1",
  "request_id": "019db438-c74a-7ef2-810c-792e298faf00",
  "idempotency_key": "browser:launch:019db438",
  "operation": {
    "kind": "launch_browser",
    "parameters": {
      "session_id": "019db438-c74a-7ef2-810c-792e298faf11",
      "browser_context_id": null
    }
  }
}"#
    );
    assert!(!json.contains("image"));
    assert!(!json.contains("mount"));
    assert!(!json.contains("privileged"));
    assert!(!json.contains("environment"));
}

#[test]
fn worker_state_response_has_stable_sanitized_shape() {
    let result = RuntimeOperationResult::WorkerState {
        execution_state: WorkerExecutionState::Exited,
        exit_code: Some(17),
    };

    let json = serde_json::to_string(&result).unwrap();

    assert_eq!(
        json,
        r#"{"state":"worker_state","execution_state":"exited","exit_code":17}"#
    );
    for forbidden in ["docker", "container", "stdout", "stderr", "secret"] {
        assert!(!json.contains(forbidden));
    }
}

#[test]
fn request_rejects_unknown_docker_shaped_fields() {
    let json = r#"{
      "api_version":"v1",
      "request_id":"019db438-c74a-7ef2-810c-792e298faf00",
      "idempotency_key":"browser:launch:019db438",
      "operation":{
        "kind":"launch_browser",
        "parameters":{
          "session_id":"019db438-c74a-7ef2-810c-792e298faf11",
          "browser_context_id":null,
          "privileged":true
        }
      }
    }"#;

    assert!(serde_json::from_str::<RuntimeOperationRequest>(json).is_err());
}

#[test]
fn browser_feature_wire_shape_contains_selections_not_runtime_material() {
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let profile_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    let extension_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf33").unwrap();
    let request = RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf00").unwrap(),
        idempotency_key: IdempotencyKey::new("browser:features:019db438").unwrap(),
        operation: RuntimeOperation::LaunchBrowser(BrowserRuntimeLaunchRequest {
            session_id,
            browser_context_id: None,
            features: BrowserRuntimeFeatures {
                network_identity: BrowserNetworkIdentity {
                    locale: Some("de-DE".to_string()),
                    ..Default::default()
                },
                egress: Some(BrowserEgressSelection {
                    profile_id,
                    proxy: Some(BrowserProxySelection {
                        url: "http://proxy.internal:3128".to_string(),
                        authentication: Some(BrowserSessionDataSource::SessionData),
                    }),
                    bypass_rules: vec!["localhost".to_string()],
                    observation_mode: BrowserEgressObservationMode::MetadataOnly,
                    custom_ca: None,
                    sensitive_log_sink_configured: false,
                }),
                extension_version_ids: vec![extension_id],
                session_file_bindings: true,
            },
        }),
    };

    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(
        value["operation"]["parameters"]["features"],
        serde_json::json!({
            "network_identity": {"locale": "de-DE"},
            "egress": {
                "profile_id": profile_id,
                "proxy": {
                    "url": "http://proxy.internal:3128",
                    "authentication": "session_data"
                },
                "bypass_rules": ["localhost"],
                "observation_mode": "metadata_only"
            },
            "extension_version_ids": [extension_id],
            "session_file_bindings": true
        })
    );
    let json = serde_json::to_string(&value).unwrap();
    for forbidden in [
        "password",
        "secret",
        "install_path",
        "host_path",
        "environment",
        "privileged",
        "docker",
    ] {
        assert!(
            !json.contains(forbidden),
            "unexpected wire field: {forbidden}"
        );
    }
}
