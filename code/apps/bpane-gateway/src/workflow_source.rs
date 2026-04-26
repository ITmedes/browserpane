use std::fs;
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSourceArchive {
    pub source: WorkflowSource,
    pub file_name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowSourceError {
    #[error("invalid workflow source: {0}")]
    Invalid(String),
    #[error("failed to resolve workflow source: {0}")]
    Resolve(String),
    #[error("failed to materialize workflow source: {0}")]
    Materialize(String),
}

#[derive(Debug, Clone)]
pub struct WorkflowSourceResolver {
    git_bin: PathBuf,
    resolve_timeout: Duration,
    materialize_timeout: Duration,
}

impl WorkflowSourceResolver {
    pub fn new(git_bin: PathBuf) -> Self {
        Self {
            git_bin,
            resolve_timeout: Duration::from_secs(15),
            materialize_timeout: Duration::from_secs(60),
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

    pub async fn materialize_archive(
        &self,
        source: &WorkflowSource,
        entrypoint: &str,
    ) -> Result<WorkflowSourceArchive, WorkflowSourceError> {
        validate_workflow_source_entrypoint(Some(source), entrypoint)?;
        let resolved_source = self.resolve(Some(source.clone())).await?.ok_or_else(|| {
            WorkflowSourceError::Invalid("workflow source is required".to_string())
        })?;
        match resolved_source {
            WorkflowSource::Git(source) => {
                let checkout_dir = TemporaryWorkflowSourceDir::new()?;
                self.clone_and_checkout_git_source(&source, checkout_dir.path())
                    .await?;
                let repo_root = checkout_dir.path().to_path_buf();
                let entrypoint_path = join_validated_relative_path(&repo_root, entrypoint)?;
                let entrypoint_root_path =
                    validated_relative_path("workflow entrypoint", entrypoint)?;
                if !entrypoint_path.is_file() {
                    return Err(WorkflowSourceError::Materialize(format!(
                        "workflow entrypoint {entrypoint} was not found at commit {}",
                        source.resolved_commit.as_deref().unwrap_or("unknown"),
                    )));
                }
                if let Some(root_path) = source.root_path.as_deref() {
                    let validated_root_path =
                        validated_relative_path("workflow git source root_path", root_path)?;
                    if !entrypoint_root_path.starts_with(&validated_root_path) {
                        return Err(WorkflowSourceError::Invalid(format!(
                            "workflow entrypoint {entrypoint} must live under workflow git source root_path {root_path}"
                        )));
                    }
                }
                let archive_root = match source.root_path.as_deref() {
                    Some(root_path) => join_validated_relative_path(&repo_root, root_path)?,
                    None => repo_root.clone(),
                };
                if !archive_root.exists() {
                    return Err(WorkflowSourceError::Materialize(format!(
                        "workflow source root path {} was not found at commit {}",
                        source.root_path.as_deref().unwrap_or("."),
                        source.resolved_commit.as_deref().unwrap_or("unknown"),
                    )));
                }
                let file_name = format!(
                    "workflow-source-{}.zip",
                    short_commit(source.resolved_commit.as_deref().ok_or_else(|| {
                        WorkflowSourceError::Materialize(
                            "resolved workflow git source is missing resolved_commit".to_string(),
                        )
                    })?)
                );
                let bytes = tokio::task::spawn_blocking(move || {
                    archive_workflow_source_tree(&repo_root, &archive_root)
                })
                .await
                .map_err(|error| {
                    WorkflowSourceError::Materialize(format!(
                        "workflow source archive task failed: {error}"
                    ))
                })??;
                Ok(WorkflowSourceArchive {
                    source: WorkflowSource::Git(source),
                    file_name,
                    media_type: "application/zip".to_string(),
                    bytes,
                })
            }
        }
    }

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

        let output = timeout(self.resolve_timeout, command.output())
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

    async fn clone_and_checkout_git_source(
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

    async fn run_materialize_git_command(
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
                WorkflowSourceError::Materialize(format!("failed to {context}: {error}"))
            })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(WorkflowSourceError::Materialize(format!(
            "failed to {context}: {}",
            if stderr.is_empty() {
                format!("exit status {}", output.status)
            } else {
                stderr
            }
        )))
    }
}

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

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn short_commit(commit: &str) -> &str {
    &commit[..12.min(commit.len())]
}

fn validated_relative_path(label: &str, value: &str) -> Result<PathBuf, WorkflowSourceError> {
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

fn join_validated_relative_path(root: &Path, value: &str) -> Result<PathBuf, WorkflowSourceError> {
    Ok(root.join(validated_relative_path("path", value)?))
}

fn archive_workflow_source_tree(
    repo_root: &Path,
    archive_root: &Path,
) -> Result<Vec<u8>, WorkflowSourceError> {
    let mut files = Vec::new();
    collect_archive_files(repo_root, archive_root, &mut files)?;
    if files.is_empty() {
        return Err(WorkflowSourceError::Materialize(
            "workflow source archive would be empty".to_string(),
        ));
    }
    files.sort_by(|left, right| left.1.cmp(&right.1));

    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (source_path, archive_path) in files {
        let archive_name = archive_path.to_string_lossy().replace('\\', "/");
        writer.start_file(&archive_name, options).map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to add {archive_name} to workflow source archive: {error}"
            ))
        })?;
        let bytes = fs::read(&source_path).map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to read workflow source file {}: {error}",
                source_path.display()
            ))
        })?;
        writer.write_all(&bytes).map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to write {archive_name} into workflow source archive: {error}"
            ))
        })?;
    }
    let cursor = writer.finish().map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "failed to finalize workflow source archive: {error}"
        ))
    })?;
    Ok(cursor.into_inner())
}

fn collect_archive_files(
    repo_root: &Path,
    current: &Path,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), WorkflowSourceError> {
    let metadata = fs::symlink_metadata(current).map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "failed to inspect workflow source path {}: {error}",
            current.display()
        ))
    })?;
    if metadata.is_file() {
        let archive_path = current.strip_prefix(repo_root).map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to derive workflow source archive path for {}: {error}",
                current.display()
            ))
        })?;
        files.push((current.to_path_buf(), archive_path.to_path_buf()));
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(WorkflowSourceError::Materialize(format!(
            "workflow source path {} is not a regular file or directory",
            current.display()
        )));
    }

    let mut entries = fs::read_dir(current)
        .map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to read workflow source directory {}: {error}",
                current.display()
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to enumerate workflow source directory {}: {error}",
                current.display()
            ))
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.strip_prefix(repo_root).ok().and_then(|relative| {
            relative
                .components()
                .next()
                .and_then(|component| match component {
                    Component::Normal(value) => Some(value),
                    _ => None,
                })
        }) == Some(std::ffi::OsStr::new(".git"))
        {
            continue;
        }
        collect_archive_files(repo_root, &path, files)?;
    }

    Ok(())
}

struct TemporaryWorkflowSourceDir {
    path: PathBuf,
}

impl TemporaryWorkflowSourceDir {
    fn new() -> Result<Self, WorkflowSourceError> {
        let path = std::env::temp_dir().join(format!("bpane-workflow-source-{}", Uuid::now_v7()));
        fs::create_dir_all(&path).map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to create temporary workflow source directory {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryWorkflowSourceDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::process::Command as StdCommand;

    use tempfile::tempdir;
    use zip::ZipArchive;

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

    fn git_head(cwd: &std::path::Path) -> String {
        let head = StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(cwd)
            .output()
            .unwrap();
        assert!(head.status.success());
        String::from_utf8_lossy(&head.stdout)
            .trim()
            .to_ascii_lowercase()
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
        let expected = git_head(temp.path());

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

    #[test]
    fn rejects_entrypoint_outside_workflow_root_path() {
        let error = validate_workflow_source_entrypoint(
            Some(&WorkflowSource::Git(WorkflowGitSource {
                repository_url: "https://example.com/repo.git".to_string(),
                r#ref: Some("refs/heads/main".to_string()),
                resolved_commit: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
                root_path: Some("workflows".to_string()),
            })),
            "scripts/export.ts",
        )
        .unwrap_err();
        assert!(matches!(error, WorkflowSourceError::Invalid(_)));
    }

    #[tokio::test]
    async fn materializes_git_source_archive_from_local_repository() {
        let temp = tempdir().unwrap();
        git(&["init", "--initial-branch=main"], temp.path());
        git(
            &["config", "user.email", "workflow@test.local"],
            temp.path(),
        );
        git(&["config", "user.name", "Workflow Test"], temp.path());
        fs::create_dir_all(temp.path().join("workflows/smoke")).unwrap();
        fs::write(temp.path().join("README.md"), "hello\n").unwrap();
        fs::write(
            temp.path().join("workflows/smoke/export.ts"),
            "export default 1;\n",
        )
        .unwrap();
        fs::write(temp.path().join("workflows/notes.txt"), "notes\n").unwrap();
        git(&["add", "."], temp.path());
        git(&["commit", "-m", "init"], temp.path());
        let head = git_head(temp.path());

        let resolver = WorkflowSourceResolver::new(PathBuf::from("git"));
        let archive = resolver
            .materialize_archive(
                &WorkflowSource::Git(WorkflowGitSource {
                    repository_url: temp.path().to_string_lossy().into_owned(),
                    r#ref: None,
                    resolved_commit: Some(head.clone()),
                    root_path: Some("workflows".to_string()),
                }),
                "workflows/smoke/export.ts",
            )
            .await
            .unwrap();

        assert_eq!(
            archive.source,
            WorkflowSource::Git(WorkflowGitSource {
                repository_url: temp.path().to_string_lossy().into_owned(),
                r#ref: None,
                resolved_commit: Some(head),
                root_path: Some("workflows".to_string()),
            })
        );
        assert_eq!(archive.media_type, "application/zip");
        assert!(archive.file_name.ends_with(".zip"));

        let mut zip = ZipArchive::new(Cursor::new(archive.bytes)).unwrap();
        let names = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        assert!(names.contains(&"workflows/smoke/export.ts".to_string()));
        assert!(names.contains(&"workflows/notes.txt".to_string()));
        assert!(!names.contains(&"README.md".to_string()));
    }
}
