use super::*;

fn id(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

fn request(operation: RuntimeOperation) -> RuntimeOperationRequest {
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: id("019db438-c74a-7ef2-810c-792e298faf00"),
        idempotency_key: IdempotencyKey::new("runtime:test:1").unwrap(),
        operation,
    }
}

fn storage_request(action: StorageHelperAction) -> StorageHelperRequest {
    StorageHelperRequest {
        action,
        session_id: None,
        source_context_id: None,
        target_context_id: None,
        declared_payload_bytes: None,
    }
}

#[test]
fn idempotency_keys_reject_unsafe_values() {
    assert_eq!(IdempotencyKey::new(""), Err(IdempotencyKeyError::Empty));
    assert_eq!(
        IdempotencyKey::new("unsafe/value"),
        Err(IdempotencyKeyError::UnsafeCharacter)
    );
    assert_eq!(
        IdempotencyKey::new("x".repeat(129)),
        Err(IdempotencyKeyError::TooLarge)
    );
}

#[test]
fn validates_browser_launch_identifiers() {
    let valid = request(RuntimeOperation::LaunchBrowser(
        BrowserRuntimeLaunchRequest {
            session_id: Uuid::now_v7(),
            browser_context_id: Some(Uuid::now_v7()),
            features: BrowserRuntimeFeatures::default(),
        },
    ));
    valid.validate().unwrap();

    let invalid = request(RuntimeOperation::LaunchBrowser(
        BrowserRuntimeLaunchRequest {
            session_id: Uuid::nil(),
            browser_context_id: None,
            features: BrowserRuntimeFeatures::default(),
        },
    ));
    assert_eq!(
        invalid.validate(),
        Err(ContractErrorCode::InvalidResourceId.into())
    );
}

#[test]
fn validates_typed_browser_feature_selections() {
    let valid = request(RuntimeOperation::LaunchBrowser(
        BrowserRuntimeLaunchRequest {
            session_id: Uuid::now_v7(),
            browser_context_id: Some(Uuid::now_v7()),
            features: BrowserRuntimeFeatures {
                network_identity: BrowserNetworkIdentity {
                    locale: Some("de-DE".to_string()),
                    languages: vec!["de-DE".to_string(), "en-US".to_string()],
                    timezone: Some("Europe/Berlin".to_string()),
                    geolocation: Some(BrowserGeolocation {
                        latitude_e7: 525_200_000,
                        longitude_e7: 134_050_000,
                        accuracy_mm: Some(25_000),
                    }),
                    user_agent: Some("BrowserPane test agent".to_string()),
                    browser_identity: Some("regulated-pilot".to_string()),
                },
                egress: Some(BrowserEgressSelection {
                    profile_id: Uuid::now_v7(),
                    proxy: Some(BrowserProxySelection {
                        url: "http://proxy.internal:3128".to_string(),
                        authentication: Some(BrowserSessionDataSource::SessionData),
                    }),
                    bypass_rules: vec!["localhost".to_string(), "*.internal".to_string()],
                    observation_mode: BrowserEgressObservationMode::TlsIntercept,
                    custom_ca: Some(BrowserSessionDataSource::SessionData),
                    sensitive_log_sink_configured: true,
                }),
                extension_version_ids: vec![Uuid::now_v7(), Uuid::now_v7()],
                session_file_bindings: true,
            },
        },
    ));

    valid.validate().unwrap();
}

#[test]
fn rejects_invalid_browser_feature_values_and_combinations() {
    let base = BrowserRuntimeLaunchRequest {
        session_id: Uuid::now_v7(),
        browser_context_id: None,
        features: BrowserRuntimeFeatures::default(),
    };
    let duplicate_extension_id = Uuid::now_v7();
    let invalid_features = [
        BrowserRuntimeFeatures {
            network_identity: BrowserNetworkIdentity {
                locale: Some("de-DE\nunsafe".to_string()),
                ..Default::default()
            },
            ..Default::default()
        },
        BrowserRuntimeFeatures {
            network_identity: BrowserNetworkIdentity {
                geolocation: Some(BrowserGeolocation {
                    latitude_e7: 900_000_001,
                    longitude_e7: 0,
                    accuracy_mm: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
        BrowserRuntimeFeatures {
            egress: Some(BrowserEgressSelection {
                profile_id: Uuid::now_v7(),
                proxy: Some(BrowserProxySelection {
                    url: "http://user:secret@proxy.internal:3128".to_string(),
                    authentication: None,
                }),
                bypass_rules: Vec::new(),
                observation_mode: BrowserEgressObservationMode::MetadataOnly,
                custom_ca: None,
                sensitive_log_sink_configured: false,
            }),
            ..Default::default()
        },
        BrowserRuntimeFeatures {
            egress: Some(BrowserEgressSelection {
                profile_id: Uuid::now_v7(),
                proxy: Some(BrowserProxySelection {
                    url: "http://proxy.internal:3128".to_string(),
                    authentication: None,
                }),
                bypass_rules: vec!["localhost;--proxy-server=other".to_string()],
                observation_mode: BrowserEgressObservationMode::MetadataOnly,
                custom_ca: None,
                sensitive_log_sink_configured: false,
            }),
            ..Default::default()
        },
        BrowserRuntimeFeatures {
            egress: Some(BrowserEgressSelection {
                profile_id: Uuid::now_v7(),
                proxy: None,
                bypass_rules: Vec::new(),
                observation_mode: BrowserEgressObservationMode::TlsIntercept,
                custom_ca: Some(BrowserSessionDataSource::SessionData),
                sensitive_log_sink_configured: true,
            }),
            ..Default::default()
        },
        BrowserRuntimeFeatures {
            extension_version_ids: vec![duplicate_extension_id, duplicate_extension_id],
            ..Default::default()
        },
    ];

    for features in invalid_features {
        let mut invalid = base.clone();
        invalid.features = features;
        assert_eq!(
            request(RuntimeOperation::LaunchBrowser(invalid)).validate(),
            Err(ContractErrorCode::InvalidOperationParameters.into())
        );
    }
}

#[test]
fn validates_storage_helper_field_combinations() {
    let session_id = Uuid::now_v7();
    let source_id = Uuid::now_v7();
    let target_id = Uuid::now_v7();
    let cases = [
        StorageHelperRequest {
            session_id: Some(session_id),
            ..storage_request(StorageHelperAction::InitializeSessionData)
        },
        StorageHelperRequest {
            session_id: Some(session_id),
            declared_payload_bytes: Some(1),
            ..storage_request(StorageHelperAction::MaterializeSessionFiles)
        },
        StorageHelperRequest {
            source_context_id: Some(source_id),
            target_context_id: Some(target_id),
            ..storage_request(StorageHelperAction::CloneBrowserContext)
        },
        StorageHelperRequest {
            source_context_id: Some(source_id),
            ..storage_request(StorageHelperAction::ExportBrowserContext)
        },
        StorageHelperRequest {
            target_context_id: Some(target_id),
            declared_payload_bytes: Some(1),
            ..storage_request(StorageHelperAction::ImportBrowserContext)
        },
        StorageHelperRequest {
            source_context_id: Some(source_id),
            ..storage_request(StorageHelperAction::MeasureBrowserContext)
        },
        StorageHelperRequest {
            source_context_id: Some(source_id),
            ..storage_request(StorageHelperAction::DeleteBrowserContext)
        },
    ];

    for storage in cases {
        request(RuntimeOperation::RunStorageHelper(storage))
            .validate()
            .unwrap();
    }
}

#[test]
fn rejects_storage_helper_ambiguity_and_invalid_payloads() {
    let mut missing_target = storage_request(StorageHelperAction::CloneBrowserContext);
    missing_target.source_context_id = Some(Uuid::now_v7());
    assert_eq!(
        request(RuntimeOperation::RunStorageHelper(missing_target)).validate(),
        Err(ContractErrorCode::InvalidOperationParameters.into())
    );

    let context_id = Uuid::now_v7();
    let same_context = StorageHelperRequest {
        source_context_id: Some(context_id),
        target_context_id: Some(context_id),
        ..storage_request(StorageHelperAction::CloneBrowserContext)
    };
    assert_eq!(
        request(RuntimeOperation::RunStorageHelper(same_context)).validate(),
        Err(ContractErrorCode::InvalidOperationParameters.into())
    );

    let missing_payload = StorageHelperRequest {
        target_context_id: Some(Uuid::now_v7()),
        ..storage_request(StorageHelperAction::ImportBrowserContext)
    };
    assert_eq!(
        request(RuntimeOperation::RunStorageHelper(missing_payload)).validate(),
        Err(ContractErrorCode::PayloadDeclarationRequired.into())
    );

    let zero_payload = StorageHelperRequest {
        target_context_id: Some(Uuid::now_v7()),
        declared_payload_bytes: Some(0),
        ..storage_request(StorageHelperAction::ImportBrowserContext)
    };
    assert_eq!(
        request(RuntimeOperation::RunStorageHelper(zero_payload)).validate(),
        Err(ContractErrorCode::PayloadDeclarationRequired.into())
    );

    let unexpected_payload = StorageHelperRequest {
        source_context_id: Some(Uuid::now_v7()),
        declared_payload_bytes: Some(1),
        ..storage_request(StorageHelperAction::ExportBrowserContext)
    };
    assert_eq!(
        request(RuntimeOperation::RunStorageHelper(unexpected_payload)).validate(),
        Err(ContractErrorCode::PayloadDeclarationNotAllowed.into())
    );
}

#[test]
fn rejects_nil_identifiers_for_every_operation_family() {
    let secret = || SecretValue::new("purpose-scoped-secret").unwrap();
    let operations = [
        RuntimeOperation::LaunchWorkflow(WorkflowWorkerLaunchRequest {
            workflow_run_id: Uuid::nil(),
            session_id: Uuid::now_v7(),
            automation_task_id: Uuid::now_v7(),
            credentials: WorkflowWorkerCredentials {
                session_automation_access_token: secret(),
                gateway_bearer_token: None,
            },
        }),
        RuntimeOperation::LaunchRecording(RecordingWorkerLaunchRequest {
            session_id: Uuid::now_v7(),
            recording_id: Uuid::nil(),
            credentials: RecordingWorkerCredentials {
                connect_ticket: secret(),
                session_automation_access_token: secret(),
                recording_worker_access_token: secret(),
            },
        }),
        RuntimeOperation::RunStorageHelper(StorageHelperRequest {
            source_context_id: Some(Uuid::nil()),
            ..storage_request(StorageHelperAction::MeasureBrowserContext)
        }),
        RuntimeOperation::ContainerLifecycle(ContainerLifecycleRequest {
            operation_kind: RuntimeOperationKind::BrowserRuntime,
            resource_id: Uuid::nil(),
            action: ContainerLifecycleAction::Inspect,
        }),
        RuntimeOperation::VolumeLifecycle(VolumeLifecycleRequest {
            operation_kind: RuntimeOperationKind::StorageHelper,
            resource_id: Uuid::nil(),
            action: VolumeLifecycleAction::Inspect,
        }),
    ];

    for operation in operations {
        assert_eq!(
            request(operation).validate(),
            Err(ContractErrorCode::InvalidResourceId.into())
        );
    }

    let mut nil_request_id = request(RuntimeOperation::LaunchBrowser(
        BrowserRuntimeLaunchRequest {
            session_id: Uuid::now_v7(),
            browser_context_id: None,
            features: BrowserRuntimeFeatures::default(),
        },
    ));
    nil_request_id.request_id = Uuid::nil();
    assert_eq!(
        nil_request_id.validate(),
        Err(ContractErrorCode::InvalidResourceId.into())
    );
}

#[test]
fn validation_errors_do_not_echo_submitted_values() {
    let submitted = Uuid::nil();
    let invalid = request(RuntimeOperation::ContainerLifecycle(
        ContainerLifecycleRequest {
            operation_kind: RuntimeOperationKind::BrowserRuntime,
            resource_id: submitted,
            action: ContainerLifecycleAction::Inspect,
        },
    ));

    let error = invalid.validate().unwrap_err();
    assert!(!error.to_string().contains(&submitted.to_string()));
}
