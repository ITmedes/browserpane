use std::fs;
use std::path::{Component, Path, PathBuf};

use reqwest::Url;

use super::{WorkflowGitSource, WorkflowSource, WorkflowSourceError, WorkflowSourcePolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ValidatedGitRepositorySource {
    Remote { protocol: String },
    TrustedLocalPath,
}

pub(super) fn validate_workflow_source_entrypoint_with_policy(
    policy: &WorkflowSourcePolicy,
    source: Option<&WorkflowSource>,
    entrypoint: &str,
) -> Result<(), WorkflowSourceError> {
    validated_relative_path("workflow entrypoint", entrypoint)?;
    if let Some(WorkflowSource::Git(source)) = source {
        validate_git_source(policy, source)?;
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

pub(super) fn validate_git_source(
    policy: &WorkflowSourcePolicy,
    source: &WorkflowGitSource,
) -> Result<(), WorkflowSourceError> {
    if source.repository_url.trim().is_empty() {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must not be empty".to_string(),
        ));
    }
    validate_git_repository_url(policy, &source.repository_url)?;
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

pub(super) fn validate_git_repository_url(
    policy: &WorkflowSourcePolicy,
    repository_url: &str,
) -> Result<ValidatedGitRepositorySource, WorkflowSourceError> {
    let trimmed = repository_url.trim();
    if trimmed.is_empty() {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must not be empty".to_string(),
        ));
    }
    if trimmed.starts_with('-') {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must not start with '-'".to_string(),
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("ext::") || lower.contains("::") {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must not use git remote helper syntax".to_string(),
        ));
    }
    if looks_like_scp_remote(trimmed) {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must use an explicit supported URL scheme"
                .to_string(),
        ));
    }
    if let Ok(url) = Url::parse(trimmed) {
        return validate_remote_git_url(&url);
    }
    validate_trusted_local_git_path(policy, trimmed)
}

fn validate_remote_git_url(url: &Url) -> Result<ValidatedGitRepositorySource, WorkflowSourceError> {
    match url.scheme() {
        "https" if url.has_host() => Ok(ValidatedGitRepositorySource::Remote {
            protocol: "https".to_string(),
        }),
        "file" => Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url file:// URLs are not supported; use a trusted local path in development"
                .to_string(),
        )),
        scheme => Err(WorkflowSourceError::Invalid(format!(
            "workflow git source repository_url scheme {scheme} is not supported"
        ))),
    }
}

fn validate_trusted_local_git_path(
    policy: &WorkflowSourcePolicy,
    repository_url: &str,
) -> Result<ValidatedGitRepositorySource, WorkflowSourceError> {
    let path = Path::new(repository_url);
    if !path.is_absolute() {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source repository_url must be an https URL or trusted absolute local path"
                .to_string(),
        ));
    }
    let canonical_path = path.canonicalize().map_err(|error| {
        WorkflowSourceError::Invalid(format!(
            "workflow git source local repository path {repository_url} is not accessible: {error}"
        ))
    })?;
    for trusted_root in policy.trusted_local_roots() {
        let canonical_root = trusted_root.canonicalize().map_err(|error| {
            WorkflowSourceError::Invalid(format!(
                "workflow git source trusted local root {} is not accessible: {error}",
                trusted_root.display()
            ))
        })?;
        if canonical_path.starts_with(canonical_root) {
            return Ok(ValidatedGitRepositorySource::TrustedLocalPath);
        }
    }
    Err(WorkflowSourceError::Invalid(format!(
        "workflow git source local repository path {repository_url} is outside configured trusted roots"
    )))
}

fn looks_like_scp_remote(value: &str) -> bool {
    if value.contains("://") {
        return false;
    }
    let Some(colon_index) = value.find(':') else {
        return false;
    };
    let first_slash = value.find('/').unwrap_or(usize::MAX);
    colon_index < first_slash
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

pub(super) fn validate_materialized_regular_file(
    repo_root: &Path,
    path: &Path,
    label: &str,
    display_path: &str,
) -> Result<(), WorkflowSourceError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "{label} {display_path} was not found or could not be inspected: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(WorkflowSourceError::Materialize(format!(
            "{label} {display_path} must be a regular file; symlinks are not supported"
        )));
    }
    if !metadata.is_file() {
        return Err(WorkflowSourceError::Materialize(format!(
            "{label} {display_path} must be a regular file"
        )));
    }
    validate_materialized_path_containment(repo_root, path, label, display_path)
}

pub(super) fn validate_materialized_directory(
    repo_root: &Path,
    path: &Path,
    label: &str,
    display_path: &str,
) -> Result<(), WorkflowSourceError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "{label} {display_path} was not found or could not be inspected: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(WorkflowSourceError::Materialize(format!(
            "{label} {display_path} must be a directory; symlinks are not supported"
        )));
    }
    if !metadata.is_dir() {
        return Err(WorkflowSourceError::Materialize(format!(
            "{label} {display_path} must be a directory"
        )));
    }
    validate_materialized_path_containment(repo_root, path, label, display_path)
}

fn validate_materialized_path_containment(
    repo_root: &Path,
    path: &Path,
    label: &str,
    display_path: &str,
) -> Result<(), WorkflowSourceError> {
    let canonical_root = repo_root.canonicalize().map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "workflow source repository root {} could not be inspected: {error}",
            repo_root.display()
        ))
    })?;
    let canonical_path = path.canonicalize().map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "{label} {display_path} could not be inspected: {error}"
        ))
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(WorkflowSourceError::Materialize(format!(
            "{label} {display_path} escapes the workflow source checkout"
        )));
    }
    Ok(())
}
