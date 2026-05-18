use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;
use tokio::time::timeout;

use super::validation::{is_commit_sha, validate_git_source};
use super::{WorkflowGitSource, WorkflowSource, WorkflowSourceError, WorkflowSourceResolver};

impl WorkflowSourceResolver {
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

    pub(super) async fn resolve_git_ref(
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

        let output = timeout(self.resolve_timeout, command.output())
            .await
            .map_err(|_| {
                WorkflowSourceError::RepositoryAccess(format!(
                    "timed out resolving git ref {ref_name} for {repository_url}"
                ))
            })?
            .map_err(|error| {
                WorkflowSourceError::Infrastructure(format!(
                    "failed to run git ls-remote for {repository_url}: {error}"
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = git_failure_detail(&stderr, output.status.to_string());
            return Err(WorkflowSourceError::RepositoryAccess(format!(
                "git ls-remote failed for {repository_url}: {}",
                with_safe_directory_hint(detail)
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

    pub(super) async fn clone_and_checkout_git_source(
        &self,
        source: &WorkflowGitSource,
        checkout_dir: &Path,
    ) -> Result<(), WorkflowSourceError> {
        let resolved_commit = source.resolved_commit.as_deref().ok_or_else(|| {
            WorkflowSourceError::Materialize(
                "resolved workflow git source is missing resolved_commit".to_string(),
            )
        })?;
        self.run_materialize_git_command(
            vec![
                "clone".to_string(),
                "--no-checkout".to_string(),
                source.repository_url.clone(),
                checkout_dir.to_string_lossy().into_owned(),
            ],
            None,
            &format!("clone repository {}", source.repository_url),
        )
        .await?;
        self.run_materialize_git_command(
            vec![
                "checkout".to_string(),
                "--detach".to_string(),
                resolved_commit.to_string(),
            ],
            Some(checkout_dir),
            &format!("checkout commit {resolved_commit}"),
        )
        .await?;
        Ok(())
    }

    pub(super) async fn run_materialize_git_command(
        &self,
        args: Vec<String>,
        cwd: Option<&Path>,
        context: &str,
    ) -> Result<(), WorkflowSourceError> {
        let mut command = Command::new(&self.git_bin);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        command
            .args(args.iter().map(String::as_str))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = timeout(self.materialize_timeout, command.output())
            .await
            .map_err(|_| {
                WorkflowSourceError::Materialize(format!("timed out attempting to {context}"))
            })?
            .map_err(|error| {
                WorkflowSourceError::Infrastructure(format!(
                    "failed to run git while attempting to {context}: {error}"
                ))
            })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail =
            with_safe_directory_hint(git_failure_detail(&stderr, output.status.to_string()));
        if matches!(args.first().map(String::as_str), Some("clone"))
            || is_safe_directory_failure(&stderr)
        {
            return Err(WorkflowSourceError::RepositoryAccess(format!(
                "failed to {context}: {detail}"
            )));
        }
        Err(WorkflowSourceError::Materialize(format!(
            "failed to {context}: {detail}"
        )))
    }
}

fn git_failure_detail(stderr: &str, fallback: String) -> String {
    if stderr.is_empty() {
        fallback
    } else {
        stderr.to_string()
    }
}

fn with_safe_directory_hint(detail: String) -> String {
    if is_safe_directory_failure(&detail) {
        format!(
            "{detail}. The local compose gateway must trust the mounted checkout; configure git safe.directory for /workspace."
        )
    } else {
        detail
    }
}

fn is_safe_directory_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("dubious ownership") || lower.contains("safe.directory")
}
