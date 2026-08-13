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
        },
    ));
    valid.validate().unwrap();

    let invalid = request(RuntimeOperation::LaunchBrowser(
        BrowserRuntimeLaunchRequest {
            session_id: Uuid::nil(),
            browser_context_id: None,
        },
    ));
    assert_eq!(
        invalid.validate(),
        Err(ContractErrorCode::InvalidResourceId.into())
    );
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
