use super::*;
use serde_json::json;

#[test]
fn rejects_non_object_integration_context() {
    let error = validate_create_request(&CreateSessionRequest {
        template_id: None,
        browser_context: None,
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
        browser_context: None,
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
fn rejects_invalid_session_template_defaults() {
    for request in [
        PersistSessionTemplateRequest {
            name: "".to_string(),
            description: None,
            labels: HashMap::new(),
            defaults: SessionTemplateDefaults::default(),
        },
        PersistSessionTemplateRequest {
            name: "template".to_string(),
            description: Some("".to_string()),
            labels: HashMap::new(),
            defaults: SessionTemplateDefaults::default(),
        },
        PersistSessionTemplateRequest {
            name: "template".to_string(),
            description: None,
            labels: HashMap::from([("".to_string(), "value".to_string())]),
            defaults: SessionTemplateDefaults::default(),
        },
        PersistSessionTemplateRequest {
            name: "template".to_string(),
            description: None,
            labels: HashMap::new(),
            defaults: SessionTemplateDefaults {
                idle_timeout_sec: Some(0),
                ..SessionTemplateDefaults::default()
            },
        },
        PersistSessionTemplateRequest {
            name: "template".to_string(),
            description: None,
            labels: HashMap::new(),
            defaults: SessionTemplateDefaults {
                integration_context: Some(Value::String("bad".to_string())),
                ..SessionTemplateDefaults::default()
            },
        },
        PersistSessionTemplateRequest {
            name: "template".to_string(),
            description: None,
            labels: HashMap::new(),
            defaults: SessionTemplateDefaults {
                recording: Some(SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(0),
                }),
                ..SessionTemplateDefaults::default()
            },
        },
    ] {
        let error = validate_session_template_request(&request).unwrap_err();
        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }
}

#[test]
fn accepts_valid_session_template_defaults() {
    validate_session_template_request(&PersistSessionTemplateRequest {
        name: "support-debug".to_string(),
        description: Some("Support debug sessions".to_string()),
        labels: HashMap::from([("team".to_string(), "support".to_string())]),
        defaults: SessionTemplateDefaults {
            owner_mode: Some(SessionOwnerMode::Collaborative),
            viewport: Some(SessionViewport {
                width: 1440,
                height: 900,
            }),
            idle_timeout_sec: Some(1800),
            labels: HashMap::from([("purpose".to_string(), "debug".to_string())]),
            integration_context: Some(json!({ "source": "template" })),
            recording: Some(SessionRecordingPolicy {
                mode: SessionRecordingMode::Manual,
                format: SessionRecordingFormat::Webm,
                retention_sec: Some(86400),
            }),
        },
    })
    .expect("valid session template defaults should pass validation");
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
