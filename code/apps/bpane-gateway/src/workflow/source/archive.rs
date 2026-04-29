use std::fs;
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};

use tokio::task;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::validation::{
    join_validated_relative_path, short_commit, validate_workflow_source_entrypoint,
    validated_relative_path,
};
use super::{WorkflowSource, WorkflowSourceArchive, WorkflowSourceError, WorkflowSourceResolver};

impl WorkflowSourceResolver {
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
                let bytes = task::spawn_blocking(move || {
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
