use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::session_control::CreateSessionRequest;

use super::WorkflowSource;

pub const PLAYWRIGHT_EXECUTOR: &str = "playwright";
pub const WORKFLOW_PACKAGE_FORMAT_V1: &str = "browserpane.workflow-package/v1";
pub const JSON_SCHEMA_DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";
pub const SUPPORTED_NODE_MAJOR_VERSION: u16 = 22;
pub const SUPPORTED_PLAYWRIGHT_MAJOR_VERSION: u16 = 1;
pub const SUPPORTED_PLAYWRIGHT_MINOR_VERSION: u16 = 59;

const MAX_PACKAGE_ID_BYTES: usize = 128;
const MAX_REVIEWER_BYTES: usize = 200;
const MAX_REQUIREMENT_IDS: usize = 64;
const MAX_EXECUTION_MARKERS: usize = 32;
const MAX_MARKER_BYTES: usize = 128;
const MAX_SCHEMA_BYTES: usize = 64 * 1024;
const MAX_SCHEMA_DEPTH: usize = 32;
const MAX_SCHEMA_NODES: usize = 4_096;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 3_600_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPackageManifest {
    pub package_id: String,
    pub format_version: String,
    pub runtime: WorkflowPackageRuntime,
    pub requirements: WorkflowPackageRequirements,
    pub execution: WorkflowPackageExecution,
    pub publication: WorkflowPackagePublication,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPackageRuntime {
    pub language: WorkflowPackageLanguage,
    pub browserpane_api_version: String,
    pub node_major_version: u16,
    pub playwright_major_version: u16,
    pub playwright_minor_version: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackageLanguage {
    #[serde(rename = "typescript")]
    TypeScript,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPackageRequirements {
    pub default_session: Value,
    #[serde(default)]
    pub allowed_credential_binding_ids: Vec<String>,
    #[serde(default)]
    pub allowed_extension_ids: Vec<String>,
    #[serde(default)]
    pub allowed_file_workspace_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPackageExecution {
    pub timeout_ms: u64,
    pub assertions: Vec<String>,
    pub safe_cancellation_points: Vec<String>,
    pub side_effect_checkpoints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPackagePublication {
    pub reviewer: String,
    pub reviewed_at: DateTime<Utc>,
    pub decision: WorkflowPackagePublicationDecision,
    pub fresh_context_replay: bool,
    pub scenarios: Vec<WorkflowPackageScenarioEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackagePublicationDecision {
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPackageScenarioEvidence {
    pub kind: WorkflowPackageScenarioKind,
    pub result: WorkflowPackageScenarioResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackageScenarioKind {
    HappyPath,
    Validation,
    MissingElement,
    AuthenticationChallenge,
    PortalFailure,
    RuntimeFailure,
    Cancellation,
    AmbiguousPostSideEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackageScenarioResult {
    Passed,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPackageCompatibilityState {
    Supported,
    Legacy,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowPackageCompatibility {
    pub state: WorkflowPackageCompatibilityState,
    pub warnings: Vec<String>,
}

impl WorkflowPackageCompatibility {
    pub fn for_version(executor: &str, package: Option<&WorkflowPackageManifest>) -> Self {
        if executor != PLAYWRIGHT_EXECUTOR {
            return Self {
                state: WorkflowPackageCompatibilityState::Unsupported,
                warnings: vec![
                    "This legacy version uses an executor that is not supported by the Phase 0 runtime."
                        .to_string(),
                ],
            };
        }
        if package.is_none() {
            return Self {
                state: WorkflowPackageCompatibilityState::Legacy,
                warnings: vec![
                    "This readable legacy version predates the Phase 0 workflow package manifest. Publish a new immutable version to adopt the supported contract."
                        .to_string(),
                ],
            };
        }
        Self {
            state: WorkflowPackageCompatibilityState::Supported,
            warnings: Vec::new(),
        }
    }
}

pub fn validate_workflow_definition_version_contract(
    executor: &str,
    entrypoint: &str,
    source: Option<&WorkflowSource>,
    input_schema: Option<&Value>,
    output_schema: Option<&Value>,
    package: Option<&WorkflowPackageManifest>,
) -> Result<(), String> {
    let Some(package) = package else {
        return Ok(());
    };
    if executor != PLAYWRIGHT_EXECUTOR {
        return Err("packaged workflow executor must be playwright".to_string());
    }
    validate_package_manifest(package)?;
    validate_typescript_entrypoint(entrypoint)?;
    validate_pinned_git_source(source)?;
    validate_declared_schema("input_schema", input_schema)?;
    validate_declared_schema("output_schema", output_schema)
}

pub fn validate_package_manifest(package: &WorkflowPackageManifest) -> Result<(), String> {
    validate_token(
        "workflow package_id",
        &package.package_id,
        MAX_PACKAGE_ID_BYTES,
    )?;
    if package.format_version != WORKFLOW_PACKAGE_FORMAT_V1 {
        return Err(format!(
            "workflow package format_version must be {WORKFLOW_PACKAGE_FORMAT_V1}"
        ));
    }
    validate_runtime(&package.runtime)?;
    validate_requirements(&package.requirements)?;
    validate_execution(&package.execution)?;
    validate_publication(&package.publication)
}

fn validate_runtime(runtime: &WorkflowPackageRuntime) -> Result<(), String> {
    if runtime.browserpane_api_version != "v1" {
        return Err("workflow package runtime browserpane_api_version must be v1".to_string());
    }
    if runtime.node_major_version != SUPPORTED_NODE_MAJOR_VERSION {
        return Err(format!(
            "workflow package runtime node_major_version must be {SUPPORTED_NODE_MAJOR_VERSION}"
        ));
    }
    if runtime.playwright_major_version != SUPPORTED_PLAYWRIGHT_MAJOR_VERSION
        || runtime.playwright_minor_version != SUPPORTED_PLAYWRIGHT_MINOR_VERSION
    {
        return Err(format!(
            "workflow package runtime Playwright version must be {SUPPORTED_PLAYWRIGHT_MAJOR_VERSION}.{SUPPORTED_PLAYWRIGHT_MINOR_VERSION}"
        ));
    }
    Ok(())
}

fn validate_requirements(requirements: &WorkflowPackageRequirements) -> Result<(), String> {
    let session =
        serde_json::from_value::<CreateSessionRequest>(requirements.default_session.clone())
            .map_err(|error| {
                format!("workflow package requirements default_session is invalid: {error}")
            })?;
    let object = requirements.default_session.as_object().ok_or_else(|| {
        "workflow package requirements default_session must be a JSON object".to_string()
    })?;
    for field in [
        "project_id",
        "browser_context",
        "network_identity",
        "capabilities",
        "recording",
    ] {
        if !object.contains_key(field) {
            return Err(format!(
                "workflow package requirements default_session must explicitly declare {field}"
            ));
        }
    }
    let network_identity = object
        .get("network_identity")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "workflow package requirements default_session network_identity must be an object"
                .to_string()
        })?;
    if !network_identity.contains_key("egress_profile_id") {
        return Err(
            "workflow package requirements default_session network_identity must explicitly declare egress_profile_id"
                .to_string(),
        );
    }
    validate_uuid_list(
        "allowed_credential_binding_ids",
        &requirements.allowed_credential_binding_ids,
    )?;
    validate_uuid_list("allowed_extension_ids", &requirements.allowed_extension_ids)?;
    validate_uuid_list(
        "allowed_file_workspace_ids",
        &requirements.allowed_file_workspace_ids,
    )?;
    if !session.extension_ids.iter().all(|id| {
        requirements
            .allowed_extension_ids
            .iter()
            .any(|allowed| allowed == &id.to_string())
    }) {
        return Err(
            "workflow package default_session extension_ids must be included in allowed_extension_ids"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_uuid_list(label: &str, values: &[String]) -> Result<(), String> {
    if values.len() > MAX_REQUIREMENT_IDS {
        return Err(format!(
            "workflow package requirements {label} must contain at most {MAX_REQUIREMENT_IDS} ids"
        ));
    }
    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        Uuid::parse_str(value)
            .map_err(|_| format!("workflow package requirements {label} entries must be UUIDs"))?;
        if !unique.insert(value) {
            return Err(format!(
                "workflow package requirements {label} must not contain duplicate ids"
            ));
        }
    }
    Ok(())
}

fn validate_execution(execution: &WorkflowPackageExecution) -> Result<(), String> {
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&execution.timeout_ms) {
        return Err(format!(
            "workflow package execution timeout_ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"
        ));
    }
    validate_marker_list("assertions", &execution.assertions)?;
    validate_marker_list(
        "safe_cancellation_points",
        &execution.safe_cancellation_points,
    )?;
    validate_marker_list(
        "side_effect_checkpoints",
        &execution.side_effect_checkpoints,
    )
}

fn validate_marker_list(label: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.len() > MAX_EXECUTION_MARKERS {
        return Err(format!(
            "workflow package execution {label} must contain between 1 and {MAX_EXECUTION_MARKERS} markers"
        ));
    }
    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        validate_token(
            &format!("workflow package execution {label} marker"),
            value,
            MAX_MARKER_BYTES,
        )?;
        if !unique.insert(value) {
            return Err(format!(
                "workflow package execution {label} must not contain duplicate markers"
            ));
        }
    }
    Ok(())
}

fn validate_publication(publication: &WorkflowPackagePublication) -> Result<(), String> {
    let reviewer = publication.reviewer.trim();
    if reviewer.is_empty() || reviewer.len() > MAX_REVIEWER_BYTES {
        return Err(format!(
            "workflow package publication reviewer must contain between 1 and {MAX_REVIEWER_BYTES} bytes"
        ));
    }
    if !publication.fresh_context_replay {
        return Err(
            "workflow package publication fresh_context_replay must be true before approval"
                .to_string(),
        );
    }
    let expected = [
        WorkflowPackageScenarioKind::HappyPath,
        WorkflowPackageScenarioKind::Validation,
        WorkflowPackageScenarioKind::MissingElement,
        WorkflowPackageScenarioKind::AuthenticationChallenge,
        WorkflowPackageScenarioKind::PortalFailure,
        WorkflowPackageScenarioKind::RuntimeFailure,
        WorkflowPackageScenarioKind::Cancellation,
        WorkflowPackageScenarioKind::AmbiguousPostSideEffect,
    ];
    let mut seen = HashSet::with_capacity(publication.scenarios.len());
    for scenario in &publication.scenarios {
        if !seen.insert(scenario.kind) {
            return Err(
                "workflow package publication scenarios must not contain duplicate kinds"
                    .to_string(),
            );
        }
        if scenario.kind == WorkflowPackageScenarioKind::HappyPath
            && scenario.result != WorkflowPackageScenarioResult::Passed
        {
            return Err(
                "workflow package publication happy_path scenario must have passed".to_string(),
            );
        }
    }
    if expected.iter().any(|kind| !seen.contains(kind)) {
        return Err(
            "workflow package publication must record every Phase 0 regression scenario kind"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_typescript_entrypoint(entrypoint: &str) -> Result<(), String> {
    let lower = entrypoint.to_ascii_lowercase();
    if !lower.ends_with(".ts") && !lower.ends_with(".tsx") {
        return Err(
            "supported workflow package entrypoint must be TypeScript (.ts or .tsx)".to_string(),
        );
    }
    Ok(())
}

fn validate_pinned_git_source(source: Option<&WorkflowSource>) -> Result<(), String> {
    let Some(WorkflowSource::Git(source)) = source else {
        return Err("supported workflow package source must be Git-backed".to_string());
    };
    let commit = source.resolved_commit.as_deref().ok_or_else(|| {
        "supported workflow package source must contain resolved_commit".to_string()
    })?;
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(
            "supported workflow package source resolved_commit must be a 40-character hex sha"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_declared_schema(label: &str, schema: Option<&Value>) -> Result<(), String> {
    let schema = schema.ok_or_else(|| {
        format!("supported workflow package {label} must declare JSON Schema Draft 2020-12")
    })?;
    let object = schema
        .as_object()
        .ok_or_else(|| format!("supported workflow package {label} must be a JSON object"))?;
    if object.get("$schema").and_then(Value::as_str) != Some(JSON_SCHEMA_DRAFT_2020_12) {
        return Err(format!(
            "supported workflow package {label} must declare JSON Schema Draft 2020-12 at $schema={JSON_SCHEMA_DRAFT_2020_12}"
        ));
    }
    let encoded = serde_json::to_vec(schema)
        .map_err(|error| format!("supported workflow package {label} is invalid: {error}"))?;
    if encoded.len() > MAX_SCHEMA_BYTES {
        return Err(format!(
            "supported workflow package {label} must not exceed {MAX_SCHEMA_BYTES} bytes"
        ));
    }
    let mut nodes = 0;
    validate_schema_shape(label, schema, 0, &mut nodes)
}

fn validate_schema_shape(
    label: &str,
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), String> {
    *nodes += 1;
    if *nodes > MAX_SCHEMA_NODES {
        return Err(format!(
            "supported workflow package {label} must not exceed {MAX_SCHEMA_NODES} JSON nodes"
        ));
    }
    if depth > MAX_SCHEMA_DEPTH {
        return Err(format!(
            "supported workflow package {label} must not exceed {MAX_SCHEMA_DEPTH} levels"
        ));
    }
    match value {
        Value::Array(values) => {
            for value in values {
                validate_schema_shape(label, value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_schema_shape(label, value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_token(label: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_bytes {
        return Err(format!(
            "{label} must contain between 1 and {max_bytes} bytes"
        ));
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
    {
        return Err(format!(
            "{label} may contain only ASCII letters, digits, '.', '_', '-', and '/'"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
