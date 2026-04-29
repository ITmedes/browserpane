use super::*;

#[test]
fn rejects_non_object_integration_context() {
    let error = validate_create_request(&CreateSessionRequest {
        template_id: None,
        owner_mode: None,
        viewport: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: Some(Value::String("bad".to_string())),
        extension_ids: Vec::new(),
        extensions: Vec::new(),
        recording: SessionRecordingPolicy::default(),
    })
    .unwrap_err();

    assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
}

#[test]
fn rejects_zero_recording_retention() {
    let error = validate_create_request(&CreateSessionRequest {
        template_id: None,
        owner_mode: None,
        viewport: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: None,
        extension_ids: Vec::new(),
        extensions: Vec::new(),
        recording: SessionRecordingPolicy {
            mode: SessionRecordingMode::Manual,
            format: SessionRecordingFormat::Webm,
            retention_sec: Some(0),
        },
    })
    .unwrap_err();

    assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
}

#[test]
fn rejects_empty_recording_failure_message() {
    let error = validate_fail_recording_request(&FailSessionRecordingRequest {
        error: "".to_string(),
        termination_reason: None,
    })
    .unwrap_err();

    assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
}
