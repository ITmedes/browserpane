use super::*;
use serde_json::json;

#[test]
fn rejects_non_object_integration_context() {
    let error = validate_create_request(&CreateSessionRequest {
        template_id: None,
        browser_context: None,
        network_identity: None,
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
        network_identity: None,
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
            network_identity: None,
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
fn validates_network_identity_and_egress_profile_shapes() {
    validate_create_request(&CreateSessionRequest {
        network_identity: Some(SessionNetworkIdentity {
            locale: Some("de-DE".to_string()),
            languages: vec!["de-DE".to_string(), "en-US".to_string()],
            timezone: Some("Europe/Berlin".to_string()),
            geolocation: Some(SessionGeolocation {
                latitude: 52.52,
                longitude: 13.405,
                accuracy_meters: Some(100.0),
            }),
            user_agent: Some("BrowserPaneTest/1.0".to_string()),
            browser_identity: Some("desktop-chromium-stable".to_string()),
            egress_profile_id: Some(Uuid::now_v7()),
        }),
        ..CreateSessionRequest::default()
    })
    .expect("valid network identity should pass validation");

    for identity in [
        SessionNetworkIdentity {
            locale: Some("bad locale".to_string()),
            ..SessionNetworkIdentity::default()
        },
        SessionNetworkIdentity {
            timezone: Some("bad timezone".to_string()),
            ..SessionNetworkIdentity::default()
        },
        SessionNetworkIdentity {
            geolocation: Some(SessionGeolocation {
                latitude: 91.0,
                longitude: 13.405,
                accuracy_meters: None,
            }),
            ..SessionNetworkIdentity::default()
        },
        SessionNetworkIdentity {
            user_agent: Some("bad\nagent".to_string()),
            ..SessionNetworkIdentity::default()
        },
        SessionNetworkIdentity {
            egress_profile_id: Some(Uuid::nil()),
            ..SessionNetworkIdentity::default()
        },
    ] {
        let error = validate_create_request(&CreateSessionRequest {
            network_identity: Some(identity),
            ..CreateSessionRequest::default()
        })
        .unwrap_err();
        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }

    validate_egress_profile_request(&PersistEgressProfileRequest {
        name: "eu-support-egress".to_string(),
        description: Some("EU support egress".to_string()),
        labels: HashMap::from([("region".to_string(), "eu".to_string())]),
        proxy: Some(EgressProxyConfig {
            url: "https://proxy.example:8443".to_string(),
            credential_binding_id: None,
        }),
        bypass_rules: vec!["localhost".to_string()],
        custom_ca: Some(EgressCustomCaConfig {
            certificate_ref: "vault://pki/browserpane/eu-support".to_string(),
            display_name: Some("EU support CA".to_string()),
        }),
        traffic_observation: EgressTrafficObservationConfig {
            mode: EgressTrafficObservationMode::TlsIntercept,
            sensitive_log_sink_ref: Some("siem://browserpane/eu-support".to_string()),
            sensitive_log_sink_display_name: Some("EU support SIEM".to_string()),
        },
        state: EgressProfileState::Ready,
    })
    .expect("valid egress profile should pass validation");

    for request in [
        PersistEgressProfileRequest {
            name: "".to_string(),
            description: None,
            labels: HashMap::new(),
            proxy: None,
            bypass_rules: Vec::new(),
            custom_ca: None,
            traffic_observation: EgressTrafficObservationConfig::default(),
            state: EgressProfileState::Ready,
        },
        PersistEgressProfileRequest {
            name: "bad-proxy".to_string(),
            description: None,
            labels: HashMap::new(),
            proxy: Some(EgressProxyConfig {
                url: "https://user:pass@proxy.example:8443".to_string(),
                credential_binding_id: None,
            }),
            bypass_rules: Vec::new(),
            custom_ca: None,
            traffic_observation: EgressTrafficObservationConfig::default(),
            state: EgressProfileState::Ready,
        },
        PersistEgressProfileRequest {
            name: "bad-ca".to_string(),
            description: None,
            labels: HashMap::new(),
            proxy: None,
            bypass_rules: Vec::new(),
            custom_ca: Some(EgressCustomCaConfig {
                certificate_ref: "".to_string(),
                display_name: None,
            }),
            traffic_observation: EgressTrafficObservationConfig::default(),
            state: EgressProfileState::Ready,
        },
        PersistEgressProfileRequest {
            name: "tls-without-sink".to_string(),
            description: None,
            labels: HashMap::new(),
            proxy: Some(EgressProxyConfig {
                url: "https://proxy.example:8443".to_string(),
                credential_binding_id: None,
            }),
            bypass_rules: Vec::new(),
            custom_ca: Some(EgressCustomCaConfig {
                certificate_ref: "file:///workspace/dev/egress-ca.pem".to_string(),
                display_name: None,
            }),
            traffic_observation: EgressTrafficObservationConfig {
                mode: EgressTrafficObservationMode::TlsIntercept,
                sensitive_log_sink_ref: None,
                sensitive_log_sink_display_name: None,
            },
            state: EgressProfileState::Ready,
        },
    ] {
        let error = validate_egress_profile_request(&request).unwrap_err();
        assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
    }
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
