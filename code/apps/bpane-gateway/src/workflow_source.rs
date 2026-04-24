use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowSource {
    Git(WorkflowGitSource),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowGitSource {
    pub repository_url: String,
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub resolved_commit: Option<String>,
    #[serde(default)]
    pub root_path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowSourceError {
    #[error("invalid workflow source: {0}")]
    Invalid(String),
    #[error("failed to resolve workflow source: {0}")]
    Resolve(String),
}

#[derive(Debug, Clone)]
pub struct WorkflowSourceResolver {
    git_bin: PathBuf,
    timeout: Duration,
}

impl WorkflowSourceResolver {
    pub fn new(git_bin: PathBuf) -> Self {
        Self {
            git_bin,
            timeout: Duration::from_secs(15),
        }
    }

    pub async fn resolve(
        &self,
        source: Option<WorkflowSource>,
    ) -> Result<Option<WorkflowSource>, WorkflowSourceError> {
        match source {
            None => Ok(None),
            Some(WorkflowSource::Git(source)) => {
                validate_git_source(&source)?;
                if source.resolved_commit.is_some() {
                    return Ok(Some(WorkflowSource::Git(source)));
                }
                let Some(ref_name) = source.r#ref.as_deref() else {
                    return Err(WorkflowSourceError::Invalid(
                        "workflow git source requires ref or resolved_commit".to_string(),
                    ));
                };
                let resolved_commit = self
                    .resolve_git_ref(&source.repository_url, ref_name)
                    .await?;
                Ok(Some(WorkflowSource::Git(WorkflowGitSource {
                    resolved_commit: Some(resolved_commit),
                    ..source
                })))
            }
        }
    }
}

fn validate_git_source(source: &WorkflowGitSource) -> Result<(), WorkflowSourceError> {
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
    if source
        .root_path
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(WorkflowSourceError::Invalid(
            "workflow git source root_path must not be empty when provided".to_string(),
        ));
    }
    Ok(())
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl WorkflowSourceResolver {
    async fn resolve_git_ref(
        &self,
        repository_url: &str,
        ref_name: &str,
    ) -> Result<String, WorkflowSourceError> {
        let mut command = Command::new(&self.git_bin);
        command
            .arg("ls-remote")
            .arg(repository_url)
            .arg(ref_name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = timeout(self.timeout, command.output())
            .await
            .map_err(|_| {
                WorkflowSourceError::Resolve(format!(
                    "timed out resolving git ref {ref_name} for {repository_url}"
                ))
            })?
            .map_err(|error| {
                WorkflowSourceError::Resolve(format!(
                    "failed to run git ls-remote for {repository_url}: {error}"
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WorkflowSourceError::Resolve(format!(
                "git ls-remote failed for {repository_url}: {}",
                if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                }
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| {
                WorkflowSourceError::Resolve(format!(
                    "git ref {ref_name} was not found in repository {repository_url}"
                ))
            })?;
        let commit = line.split_whitespace().next().ok_or_else(|| {
            WorkflowSourceError::Resolve(format!(
                "git ls-remote returned malformed output for {repository_url}"
            ))
        })?;
        if !is_commit_sha(commit) {
            return Err(WorkflowSourceError::Resolve(format!(
                "git ls-remote returned invalid commit sha for {repository_url}"
            )));
        }
        Ok(commit.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command as StdCommand;

    use tempfile::tempdir;

    use super::*;

    fn git(args: &[&str], cwd: &std::path::Path) {
        let output = StdCommand::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn preserves_explicit_resolved_commit_without_git_lookup() {
        let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
        let source = WorkflowSource::Git(WorkflowGitSource {
            repository_url: "https://example.com/repo.git".to_string(),
            r#ref: None,
            resolved_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
            root_path: Some("workflows".to_string()),
        });

        let resolved = resolver.resolve(Some(source.clone())).await.unwrap();
        assert_eq!(resolved, Some(source));
    }

    #[tokio::test]
    async fn resolves_git_source_from_local_repository_ref() {
        let temp = tempdir().unwrap();
        git(&["init", "--initial-branch=main"], temp.path());
        git(
            &["config", "user.email", "workflow@test.local"],
            temp.path(),
        );
        git(&["config", "user.name", "Workflow Test"], temp.path());
        fs::write(temp.path().join("README.md"), "hello\n").unwrap();
        git(&["add", "README.md"], temp.path());
        git(&["commit", "-m", "init"], temp.path());
        let head = StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(head.status.success());
        let expected = String::from_utf8_lossy(&head.stdout)
            .trim()
            .to_ascii_lowercase();

        let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
        let resolved = resolver
            .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
                repository_url: temp.path().to_string_lossy().into_owned(),
                r#ref: Some("HEAD".to_string()),
                resolved_commit: None,
                root_path: Some("workflows".to_string()),
            })))
            .await
            .unwrap();

        assert_eq!(
            resolved,
            Some(WorkflowSource::Git(WorkflowGitSource {
                repository_url: temp.path().to_string_lossy().into_owned(),
                r#ref: Some("HEAD".to_string()),
                resolved_commit: Some(expected),
                root_path: Some("workflows".to_string()),
            }))
        );
    }

    #[tokio::test]
    async fn rejects_git_source_without_ref_or_commit() {
        let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
        let error = resolver
            .resolve(Some(WorkflowSource::Git(WorkflowGitSource {
                repository_url: "https://example.com/repo.git".to_string(),
                r#ref: None,
                resolved_commit: None,
                root_path: None,
            })))
            .await
            .unwrap_err();
        assert!(matches!(error, WorkflowSourceError::Invalid(_)));
    }
}
