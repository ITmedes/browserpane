use jsonschema::Draft;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

const MAX_SCHEMA_VIOLATIONS: usize = 8;
const MAX_POINTER_LENGTH: usize = 256;
const MAX_MESSAGE_LENGTH: usize = 512;
const DRAFT_2020_12_URI: &str = "https://json-schema.org/draft/2020-12/schema";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowSchemaViolation {
    pub pointer: String,
    pub message: String,
}

pub fn validate_endpoint_schema(schema: &Value) -> Result<(), Vec<WorkflowSchemaViolation>> {
    if let Some(declared_draft) = schema.get("$schema").and_then(Value::as_str) {
        if declared_draft.trim_end_matches('#') != DRAFT_2020_12_URI {
            return Err(vec![WorkflowSchemaViolation {
                pointer: "/$schema".to_string(),
                message: "schema must declare JSON Schema Draft 2020-12".to_string(),
            }]);
        }
    }
    let meta_validator = jsonschema::draft202012::meta::validator();
    let meta_errors = meta_validator
        .iter_errors(schema)
        .take(MAX_SCHEMA_VIOLATIONS)
        .map(|error| violation(error.instance_path().as_str(), error.to_string()))
        .collect::<Vec<_>>();
    if !meta_errors.is_empty() {
        return Err(meta_errors);
    }
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(schema)
        .map(|_| ())
        .map_err(|error| vec![violation("", error.to_string())])
}

pub fn validate_schema_instance(
    schema: &Value,
    instance: &Value,
) -> Result<(), Vec<WorkflowSchemaViolation>> {
    let validator = jsonschema::options()
        .with_draft(Draft::Draft202012)
        .build(schema)
        .map_err(|error| vec![violation("", error.to_string())])?;
    let errors = validator
        .iter_errors(instance)
        .take(MAX_SCHEMA_VIOLATIONS)
        .map(|error| violation(error.instance_path().as_str(), error.to_string()))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn canonical_request_fingerprint(value: &Value) -> Result<String, serde_json::Error> {
    let normalized = canonicalize_json(value);
    serde_json::to_vec(&normalized).map(|bytes| hex::encode(Sha256::digest(bytes)))
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut normalized = serde_json::Map::new();
            for key in keys {
                if let Some(entry) = object.get(key) {
                    normalized.insert(key.clone(), canonicalize_json(entry));
                }
            }
            Value::Object(normalized)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_json).collect()),
        other => other.clone(),
    }
}

fn violation(pointer: &str, message: String) -> WorkflowSchemaViolation {
    WorkflowSchemaViolation {
        pointer: truncate(pointer, MAX_POINTER_LENGTH),
        message: truncate(&message, MAX_MESSAGE_LENGTH),
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}
