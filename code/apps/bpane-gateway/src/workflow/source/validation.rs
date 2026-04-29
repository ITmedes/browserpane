use std::path::{Component, Path, PathBuf};

use super::{WorkflowGitSource, WorkflowSource, WorkflowSourceError};

pub fn validate_workflow_source_entrypoint(
    source: Option<&WorkflowSource>,
    entrypoint: &str,
) -> Result<(), WorkflowSourceError> {
    validated_relative_path("workflow entrypoint", entrypoint)?;
    if let Some(WorkflowSource::Git(source)) = source {
        validate_git_source(source)?;
        if let Some(root_path) = source.root_path.as_deref() {
            let validated_root_path =
                validated_relative_path("workflow git source root_path", root_path)?;
            let validated_entrypoint = validated_relative_path("workflow entrypoint", entrypoint)?;
            if !validated_entrypoint.starts_with(&validated_root_path) {
                return Err(WorkflowSourceError::Invalid(format!(
                    "workflow entrypoint {entrypoint} must live under workflow git source root_path {root_path}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_git_source(source: &WorkflowGitSource) -> Result<(), WorkflowSourceError> {
    if source.repository_url.trim().is_empty() {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must not be empty".to_string(),
        ));
    }
    if source
        .r#ref
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source ref must not be empty when provided".to_string(),
        ));
    }
    if let Some(commit) = source.resolved_commit.as_deref() {
        if !is_commit_sha(commit) {
            return Err(WorkflowSourceError::Invalid(
                "workflow git source resolved_commit must be a 40-character hex sha".to_string(),
            ));
        }
    }
    if let Some(root_path) = source.root_path.as_deref() {
        if root_path.trim().is_empty() {
            return Err(WorkflowSourceError::Invalid(
                "workflow git source root_path must not be empty when provided".to_string(),
            ));
        }
        validated_relative_path("workflow git source root_path", root_path)?;
    }
    Ok(())
}

pub(super) fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn short_commit(commit: &str) -> &str {
    &commit[..12.min(commit.len())]
}

pub(super) fn validated_relative_path(
    label: &str,
    value: &str,
) -> Result<PathBuf, WorkflowSourceError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(WorkflowSourceError::Invalid(format!(
            "{label} must not be empty"
        )));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(WorkflowSourceError::Invalid(format!(
            "{label} must be a relative path"
        )));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            _ => {
                return Err(WorkflowSourceError::Invalid(format!(
                    "{label} must only contain normal path components"
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(WorkflowSourceError::Invalid(format!(
            "{label} must not be empty"
        )));
    }
    Ok(normalized)
}

pub(super) fn join_validated_relative_path(
    root: &Path,
    value: &str,
) -> Result<PathBuf, WorkflowSourceError> {
    Ok(root.join(validated_relative_path("path", value)?))
}
