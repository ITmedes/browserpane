use bpane_runtime_contract::{
    BrokerApiVersion, BrowserRuntimeLaunchRequest, IdempotencyKey, RuntimeOperation,
    RuntimeOperationRequest,
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
