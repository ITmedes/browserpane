use std::fs;
use std::path::{Component, Path, PathBuf};

use tokio::io::AsyncReadExt;
use tokio::task;

use super::archive::TemporaryWorkflowSourceDir;
use super::validation::{
    join_validated_relative_path, validate_workflow_source_entrypoint_with_policy,
    validated_relative_path,
};
use super::{
    WorkflowGitSource, WorkflowSource, WorkflowSourceError, WorkflowSourceFile,
    WorkflowSourceFileListing, WorkflowSourcePreview, WorkflowSourceResolver,
};

impl WorkflowSourceResolver {
    pub async fn materialize_source_files(
        &self,
        source: &WorkflowSource,
        entrypoint: &str,
    ) -> Result<WorkflowSourceFileListing, WorkflowSourceError> {
        let checkout = self.checkout_workflow_source(source, entrypoint).await?;
        let repo_root = checkout.repo_root.clone();
        let archive_root = checkout.archive_root.clone();
        let entrypoint = entrypoint.to_string();
        let files = task::spawn_blocking(move || {
            collect_source_files(&repo_root, &archive_root, &entrypoint)
        })
        .await
        .map_err(|error| {
            WorkflowSourceError::Snapshot(format!(
                "workflow source file listing task failed: {error}"
            ))
        })??;
        Ok(WorkflowSourceFileListing {
            source: WorkflowSource::Git(checkout.git_source),
            files,
        })
    }

    pub async fn materialize_source_file_preview(
        &self,
        source: &WorkflowSource,
        entrypoint: &str,
        source_path: &str,
        max_bytes: usize,
    ) -> Result<WorkflowSourcePreview, WorkflowSourceError> {
        if max_bytes == 0 {
            return Err(WorkflowSourceError::Invalid(
                "workflow source preview max_bytes must be greater than zero".to_string(),
            ));
        }
        validate_source_path_under_root(source, source_path)?;
        let checkout = self.checkout_workflow_source(source, entrypoint).await?;
        let selected_path = join_validated_relative_path(&checkout.repo_root, source_path)?;
        if !selected_path.is_file() {
            return Err(WorkflowSourceError::Materialize(format!(
                "workflow source file {source_path} was not found at commit {}",
                checkout
                    .git_source
                    .resolved_commit
                    .as_deref()
                    .unwrap_or("unknown"),
            )));
        }
        let (content, byte_count, truncated) = read_text_preview(&selected_path, max_bytes).await?;
        Ok(WorkflowSourcePreview {
            source: WorkflowSource::Git(checkout.git_source),
            entrypoint: entrypoint.to_string(),
            path: source_path.to_string(),
            media_type: media_type_for_path(source_path).to_string(),
            language: language_for_path(source_path).to_string(),
            content,
            byte_count,
            truncated,
        })
    }

    async fn checkout_workflow_source(
        &self,
        source: &WorkflowSource,
        entrypoint: &str,
    ) -> Result<MaterializedWorkflowSource, WorkflowSourceError> {
        validate_workflow_source_entrypoint_with_policy(
            &self.source_policy,
            Some(source),
            entrypoint,
        )?;
        let resolved_source = self.resolve(Some(source.clone())).await?.ok_or_else(|| {
            WorkflowSourceError::Invalid("workflow source is required".to_string())
        })?;
        match resolved_source {
            WorkflowSource::Git(git_source) => {
                let checkout_dir = TemporaryWorkflowSourceDir::new()?;
                self.clone_and_checkout_git_source(&git_source, checkout_dir.path())
                    .await?;
                let repo_root = checkout_dir.path().to_path_buf();
                let entrypoint_path = join_validated_relative_path(&repo_root, entrypoint)?;
                if !entrypoint_path.is_file() {
                    return Err(WorkflowSourceError::Materialize(format!(
                        "workflow entrypoint {entrypoint} was not found at commit {}",
                        git_source.resolved_commit.as_deref().unwrap_or("unknown"),
                    )));
                }
                let archive_root = match git_source.root_path.as_deref() {
                    Some(root_path) => join_validated_relative_path(&repo_root, root_path)?,
                    None => repo_root.clone(),
                };
                if !archive_root.exists() {
                    return Err(WorkflowSourceError::Materialize(format!(
                        "workflow source root path {} was not found at commit {}",
                        git_source.root_path.as_deref().unwrap_or("."),
                        git_source.resolved_commit.as_deref().unwrap_or("unknown"),
                    )));
                }
                Ok(MaterializedWorkflowSource {
                    _checkout_dir: checkout_dir,
                    git_source,
                    repo_root,
                    archive_root,
                })
            }
        }
    }
}

struct MaterializedWorkflowSource {
    _checkout_dir: TemporaryWorkflowSourceDir,
    git_source: WorkflowGitSource,
    repo_root: PathBuf,
    archive_root: PathBuf,
}

fn collect_source_files(
    repo_root: &Path,
    archive_root: &Path,
    entrypoint: &str,
) -> Result<Vec<WorkflowSourceFile>, WorkflowSourceError> {
    let mut files = Vec::new();
    collect_source_files_recursive(repo_root, archive_root, entrypoint, &mut files)?;
    if files.is_empty() {
        return Err(WorkflowSourceError::Snapshot(
            "workflow source file listing would be empty".to_string(),
        ));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_source_files_recursive(
    repo_root: &Path,
    current: &Path,
    entrypoint: &str,
    files: &mut Vec<WorkflowSourceFile>,
) -> Result<(), WorkflowSourceError> {
    let metadata = fs::symlink_metadata(current).map_err(|error| {
        WorkflowSourceError::Snapshot(format!(
            "failed to inspect workflow source path {}: {error}",
            current.display()
        ))
    })?;
    if metadata.is_file() {
        let source_path = current.strip_prefix(repo_root).map_err(|error| {
            WorkflowSourceError::Snapshot(format!(
                "failed to derive workflow source path for {}: {error}",
                current.display()
            ))
        })?;
        let path = source_path.to_string_lossy().replace('\\', "/");
        files.push(WorkflowSourceFile {
            byte_count: metadata.len(),
            media_type: media_type_for_path(&path).to_string(),
            language: language_for_path(&path).to_string(),
            entrypoint: path == entrypoint,
            path,
        });
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(WorkflowSourceError::Snapshot(format!(
            "workflow source path {} is not a regular file or directory",
            current.display()
        )));
    }

    let mut entries = fs::read_dir(current)
        .map_err(|error| {
            WorkflowSourceError::Snapshot(format!(
                "failed to read workflow source directory {}: {error}",
                current.display()
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            WorkflowSourceError::Snapshot(format!(
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
        collect_source_files_recursive(repo_root, &path, entrypoint, files)?;
    }

    Ok(())
}

fn validate_source_path_under_root(
    source: &WorkflowSource,
    source_path: &str,
) -> Result<(), WorkflowSourceError> {
    let selected_path = validated_relative_path("workflow source preview path", source_path)?;
    match source {
        WorkflowSource::Git(source) => {
            if let Some(root_path) = source.root_path.as_deref() {
                let root_path =
                    validated_relative_path("workflow git source root_path", root_path)?;
                if !selected_path.starts_with(root_path) {
                    return Err(WorkflowSourceError::Invalid(format!(
                        "workflow source preview path {source_path} must live under workflow git source root_path {}",
                        source.root_path.as_deref().unwrap_or_default()
                    )));
                }
            }
        }
    }
    Ok(())
}

async fn read_text_preview(
    path: &Path,
    max_bytes: usize,
) -> Result<(String, usize, bool), WorkflowSourceError> {
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "failed to open workflow source file {}: {error}",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(max_bytes.saturating_add(1));
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to read workflow source file {}: {error}",
                path.display()
            ))
        })?;
    let truncated = bytes.len() > max_bytes;
    if truncated {
        bytes.truncate(max_bytes);
        while std::str::from_utf8(&bytes).is_err() && !bytes.is_empty() {
            bytes.pop();
        }
    }
    let byte_count = bytes.len();
    let content = String::from_utf8(bytes).map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "workflow source file {} is not valid UTF-8: {error}",
            path.display()
        ))
    })?;
    Ok((content, byte_count, truncated))
}

fn language_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => "typescript",
        "json" => "json",
        _ => "text",
    }
}

fn media_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "ts" | "tsx" => "text/typescript; charset=utf-8",
        "js" | "jsx" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}
