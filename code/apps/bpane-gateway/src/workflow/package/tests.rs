use chrono::TimeZone;
use serde_json::json;

use super::*;
use crate::workflow::source::WorkflowGitSource;

#[test]
fn accepts_the_supported_phase_zero_package_contract() {
    let package = supported_package();
    let source = WorkflowSource::Git(WorkflowGitSource {
        repository_url: "https://example.com/workflow.git".to_string(),
        r#ref: Some("main".to_string()),
        resolved_commit: Some("a".repeat(40)),
        root_path: Some("workflow".to_string()),
    });
    let schema = json!({
        "$schema": JSON_SCHEMA_DRAFT_2020_12,
        "type": "object",
        "additionalProperties": false
    });

    validate_workflow_definition_version_contract(
        PLAYWRIGHT_EXECUTOR,
        "workflow/run.ts",
        Some(&source),
        Some(&schema),
        Some(&schema),
        Some(&package),
    )
    .unwrap();
}

#[test]
fn rejects_unsupported_executor_source_schema_and_runtime_contracts() {
    let schema = json!({ "$schema": JSON_SCHEMA_DRAFT_2020_12, "type": "object" });
    let source = WorkflowSource::Git(WorkflowGitSource {
        repository_url: "https://example.com/workflow.git".to_string(),
        r#ref: Some("main".to_string()),
        resolved_commit: Some("b".repeat(40)),
        root_path: None,
    });
    let package = supported_package();
    assert_eq!(
        validate_workflow_definition_version_contract(
            "shell",
            "workflow/run.ts",
            Some(&source),
            Some(&schema),
            Some(&schema),
            Some(&package),
        )
        .unwrap_err(),
        "packaged workflow executor must be playwright"
    );

    assert!(validate_workflow_definition_version_contract(
        PLAYWRIGHT_EXECUTOR,
        "workflow/run.mjs",
        Some(&source),
        Some(&schema),
        Some(&schema),
        Some(&package),
    )
    .unwrap_err()
    .contains("TypeScript"));
    assert!(validate_workflow_definition_version_contract(
        PLAYWRIGHT_EXECUTOR,
        "workflow/run.ts",
        None,
        Some(&schema),
        Some(&schema),
        Some(&package),
    )
    .unwrap_err()
    .contains("Git-backed"));

    let mut invalid_schema = schema.clone();
    invalid_schema["$schema"] = json!("http://json-schema.org/draft-07/schema#");
    assert!(validate_workflow_definition_version_contract(
        PLAYWRIGHT_EXECUTOR,
        "workflow/run.ts",
        Some(&source),
        Some(&invalid_schema),
        Some(&schema),
        Some(&package),
    )
    .unwrap_err()
    .contains("Draft 2020-12"));

    let mut invalid_runtime = supported_package();
    invalid_runtime.runtime.node_major_version = 20;
    assert!(validate_package_manifest(&invalid_runtime)
        .unwrap_err()
        .contains("node_major_version"));
}

#[test]
fn requires_complete_replay_and_explicit_resource_evidence() {
    let mut package = supported_package();
    package.publication.scenarios.pop();
    assert!(validate_package_manifest(&package)
        .unwrap_err()
        .contains("every Phase 0 regression scenario"));

    let mut package = supported_package();
    package
        .requirements
        .default_session
        .as_object_mut()
        .unwrap()
        .remove("recording");
    assert!(validate_package_manifest(&package)
        .unwrap_err()
        .contains("explicitly declare recording"));
}

#[test]
fn classifies_legacy_and_unsupported_versions_without_rewriting_them() {
    assert_eq!(
        WorkflowPackageCompatibility::for_version(PLAYWRIGHT_EXECUTOR, None).state,
        WorkflowPackageCompatibilityState::Legacy
    );
    assert_eq!(
        WorkflowPackageCompatibility::for_version("legacy-shell", None).state,
        WorkflowPackageCompatibilityState::Unsupported
    );
    assert_eq!(
        WorkflowPackageCompatibility::for_version(PLAYWRIGHT_EXECUTOR, Some(&supported_package()))
            .state,
        WorkflowPackageCompatibilityState::Supported
    );
}

#[test]
fn rejects_unknown_manifest_fields_instead_of_silently_dropping_them() {
    let mut value = serde_json::to_value(supported_package()).unwrap();
    value["unreviewed_extension"] = json!(true);

    assert!(serde_json::from_value::<WorkflowPackageManifest>(value)
        .unwrap_err()
        .to_string()
        .contains("unknown field"));
}

fn supported_package() -> WorkflowPackageManifest {
    WorkflowPackageManifest {
        package_id: "example.phase0.v1".to_string(),
        format_version: WORKFLOW_PACKAGE_FORMAT_V1.to_string(),
        runtime: WorkflowPackageRuntime {
            language: WorkflowPackageLanguage::TypeScript,
            browserpane_api_version: "v1".to_string(),
            node_major_version: SUPPORTED_NODE_MAJOR_VERSION,
            playwright_major_version: SUPPORTED_PLAYWRIGHT_MAJOR_VERSION,
            playwright_minor_version: SUPPORTED_PLAYWRIGHT_MINOR_VERSION,
        },
        requirements: WorkflowPackageRequirements {
            default_session: json!({
                "project_id": "019c888e-934b-7000-8000-000000000001",
                "browser_context": { "mode": "fresh", "context_id": null },
                "network_identity": { "egress_profile_id": null },
                "capabilities": {
                    "browser_input": true,
                    "clipboard": false,
                    "audio": false,
                    "microphone": false,
                    "camera": false,
                    "file_transfer": false,
                    "resize": true
                },
                "recording": { "mode": "disabled", "format": "webm", "retention_sec": null },
                "extension_ids": []
            }),
            allowed_credential_binding_ids: Vec::new(),
            allowed_extension_ids: Vec::new(),
            allowed_file_workspace_ids: Vec::new(),
        },
        execution: WorkflowPackageExecution {
            timeout_ms: 60_000,
            assertions: vec!["result-schema".to_string()],
            safe_cancellation_points: vec!["before-submit".to_string()],
            side_effect_checkpoints: vec!["after-submit".to_string()],
        },
        publication: WorkflowPackagePublication {
            reviewer: "workflow-reviewer".to_string(),
            reviewed_at: Utc.with_ymd_and_hms(2026, 8, 20, 12, 0, 0).unwrap(),
            decision: WorkflowPackagePublicationDecision::Approved,
            fresh_context_replay: true,
            scenarios: [
                WorkflowPackageScenarioKind::HappyPath,
                WorkflowPackageScenarioKind::Validation,
                WorkflowPackageScenarioKind::MissingElement,
                WorkflowPackageScenarioKind::AuthenticationChallenge,
                WorkflowPackageScenarioKind::PortalFailure,
                WorkflowPackageScenarioKind::RuntimeFailure,
                WorkflowPackageScenarioKind::Cancellation,
                WorkflowPackageScenarioKind::AmbiguousPostSideEffect,
            ]
            .into_iter()
            .map(|kind| WorkflowPackageScenarioEvidence {
                kind,
                result: WorkflowPackageScenarioResult::Passed,
            })
            .collect(),
        },
    }
}
