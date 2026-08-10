use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::fs;
use uuid::Uuid;

use crate::session_control::SessionRecordingFormat;

const LOCAL_FS_REF_PREFIX: &str = "local_fs:";

#[derive(Debug, Clone)]
pub struct FinalizeRecordingArtifactRequest {
    pub session_id: Uuid,
    pub recording_id: Uuid,
    pub format: SessionRecordingFormat,
    pub source_path: String,
    pub expected_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecordingArtifact {
    pub artifact_ref: String,
    pub bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordingArtifactStoreError {
    #[error("invalid artifact reference: {0}")]
    InvalidReference(String),
    #[error("invalid source path: {0}")]
    InvalidSourcePath(String),
    #[error("recording artifact byte count mismatch: declared {declared}, actual {actual}")]
    ByteCountMismatch { declared: u64, actual: u64 },
    #[error("invalid recording artifact store configuration: {0}")]
    InvalidConfiguration(String),
    #[error("recording artifact backend failed: {0}")]
    Backend(#[from] std::io::Error),
}

impl RecordingArtifactStoreError {
    pub fn io_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            Self::Backend(error) => Some(error.kind()),
            _ => None,
        }
    }
}

#[async_trait]
pub trait RecordingArtifactStoreBackend: Send + Sync {
    async fn check_readiness(&self) -> Result<(), RecordingArtifactStoreError>;

    async fn finalize(
        &self,
        request: FinalizeRecordingArtifactRequest,
    ) -> Result<StoredRecordingArtifact, RecordingArtifactStoreError>;

    async fn read(&self, artifact_ref: &str) -> Result<Vec<u8>, RecordingArtifactStoreError>;

    async fn delete(&self, artifact_ref: &str) -> Result<(), RecordingArtifactStoreError>;
}

#[derive(Clone)]
pub struct RecordingArtifactStore {
    backend: Arc<dyn RecordingArtifactStoreBackend>,
}

impl RecordingArtifactStore {
    pub fn new(backend: Arc<dyn RecordingArtifactStoreBackend>) -> Self {
        Self { backend }
    }

    pub fn local_fs(root: PathBuf, staging_root: PathBuf) -> Self {
        Self::new(Arc::new(LocalFsRecordingArtifactStore {
            root,
            staging_root,
        }))
    }

    pub async fn check_readiness(&self) -> Result<(), RecordingArtifactStoreError> {
        self.backend.check_readiness().await
    }

    pub async fn finalize(
        &self,
        request: FinalizeRecordingArtifactRequest,
    ) -> Result<StoredRecordingArtifact, RecordingArtifactStoreError> {
        self.backend.finalize(request).await
    }

    pub async fn read(&self, artifact_ref: &str) -> Result<Vec<u8>, RecordingArtifactStoreError> {
        self.backend.read(artifact_ref).await
    }

    pub async fn delete(&self, artifact_ref: &str) -> Result<(), RecordingArtifactStoreError> {
        self.backend.delete(artifact_ref).await
    }
}

#[derive(Debug)]
pub struct LocalFsRecordingArtifactStore {
    root: PathBuf,
    staging_root: PathBuf,
}

#[async_trait]
impl RecordingArtifactStoreBackend for LocalFsRecordingArtifactStore {
    async fn check_readiness(&self) -> Result<(), RecordingArtifactStoreError> {
        validate_local_store_roots(&self.root, &self.staging_root)?;
        check_local_store_root(&self.root).await?;
        check_local_store_root(&self.staging_root).await?;
        validate_canonical_store_roots(&self.root, &self.staging_root).await
    }

    async fn finalize(
        &self,
        request: FinalizeRecordingArtifactRequest,
    ) -> Result<StoredRecordingArtifact, RecordingArtifactStoreError> {
        validate_local_store_roots(&self.root, &self.staging_root)?;
        let relative =
            relative_artifact_path(request.session_id, request.recording_id, request.format);
        let source_path = self
            .validate_staged_source(&request.source_path, &relative)
            .await?;
        let source_metadata = fs::metadata(&source_path).await?;
        let source_bytes = source_metadata.len();
        if let Some(declared) = request.expected_bytes {
            if declared != source_bytes {
                return Err(RecordingArtifactStoreError::ByteCountMismatch {
                    declared,
                    actual: source_bytes,
                });
            }
        }
        let destination = self.root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }

        match fs::rename(&source_path, &destination).await {
            Ok(()) => {}
            Err(error) if should_fallback_to_copy(&error) => {
                fs::copy(&source_path, &destination).await?;
                if let Err(error) = fs::remove_file(&source_path).await {
                    if !should_ignore_source_cleanup_error(&error) {
                        return Err(error.into());
                    }
                }
            }
            Err(error) => return Err(error.into()),
        }

        let bytes = fs::metadata(&destination).await?.len();
        if let Some(declared) = request.expected_bytes {
            if declared != bytes {
                let _ = fs::remove_file(&destination).await;
                return Err(RecordingArtifactStoreError::ByteCountMismatch {
                    declared,
                    actual: bytes,
                });
            }
        }

        Ok(StoredRecordingArtifact {
            artifact_ref: format!("{LOCAL_FS_REF_PREFIX}{}", relative.to_string_lossy()),
            bytes,
        })
    }

    async fn read(&self, artifact_ref: &str) -> Result<Vec<u8>, RecordingArtifactStoreError> {
        let path = self.resolve_path(artifact_ref)?;
        Ok(fs::read(path).await?)
    }

    async fn delete(&self, artifact_ref: &str) -> Result<(), RecordingArtifactStoreError> {
        let path = self.resolve_path(artifact_ref)?;
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

async fn check_local_store_root(root: &Path) -> Result<(), RecordingArtifactStoreError> {
    fs::create_dir_all(root).await?;
    let probe_path = root.join(format!(".bpane-readiness-{}", Uuid::now_v7()));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
        .await?;
    drop(file);
    fs::remove_file(probe_path).await?;
    Ok(())
}

impl LocalFsRecordingArtifactStore {
    async fn validate_staged_source(
        &self,
        source_path: &str,
        relative: &Path,
    ) -> Result<PathBuf, RecordingArtifactStoreError> {
        let source_path = validate_source_path(source_path)?;
        let expected_path = self.staging_root.join(relative);
        if source_path != expected_path {
            return Err(RecordingArtifactStoreError::InvalidSourcePath(
                "source path does not match the assigned recording staging path".to_string(),
            ));
        }

        let metadata = fs::symlink_metadata(&source_path).await.map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RecordingArtifactStoreError::InvalidSourcePath(
                    "assigned recording artifact does not exist".to_string(),
                )
            } else {
                error.into()
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(RecordingArtifactStoreError::InvalidSourcePath(
                "assigned recording artifact must not be a symbolic link".to_string(),
            ));
        }
        if !metadata.is_file() {
            return Err(RecordingArtifactStoreError::InvalidSourcePath(
                "assigned recording artifact must be a regular file".to_string(),
            ));
        }

        let canonical_staging_root = fs::canonicalize(&self.staging_root).await?;
        let canonical_source = fs::canonicalize(&source_path).await?;
        if canonical_source != canonical_staging_root.join(relative) {
            return Err(RecordingArtifactStoreError::InvalidSourcePath(
                "assigned recording artifact contains a symbolic-link or path alias".to_string(),
            ));
        }
        Ok(source_path)
    }

    fn resolve_path(&self, artifact_ref: &str) -> Result<PathBuf, RecordingArtifactStoreError> {
        let relative = artifact_ref
            .strip_prefix(LOCAL_FS_REF_PREFIX)
            .ok_or_else(|| {
                RecordingArtifactStoreError::InvalidReference(artifact_ref.to_string())
            })?;
        let path = Path::new(relative);
        if path.as_os_str().is_empty() {
            return Err(RecordingArtifactStoreError::InvalidReference(
                "artifact reference path must not be empty".to_string(),
            ));
        }
        for component in path.components() {
            match component {
                Component::Normal(_) => {}
                _ => {
                    return Err(RecordingArtifactStoreError::InvalidReference(
                        artifact_ref.to_string(),
                    ));
                }
            }
        }
        Ok(self.root.join(path))
    }
}

fn validate_local_store_roots(
    root: &Path,
    staging_root: &Path,
) -> Result<(), RecordingArtifactStoreError> {
    if !is_normal_absolute_path(root) || !is_normal_absolute_path(staging_root) {
        return Err(RecordingArtifactStoreError::InvalidConfiguration(
            "finalized and staging roots must be absolute and contain no path aliases".to_string(),
        ));
    }
    if root == staging_root || root.starts_with(staging_root) || staging_root.starts_with(root) {
        return Err(RecordingArtifactStoreError::InvalidConfiguration(
            "finalized and staging roots must be distinct and non-overlapping".to_string(),
        ));
    }
    Ok(())
}

async fn validate_canonical_store_roots(
    root: &Path,
    staging_root: &Path,
) -> Result<(), RecordingArtifactStoreError> {
    let root = fs::canonicalize(root).await?;
    let staging_root = fs::canonicalize(staging_root).await?;
    if root == staging_root || root.starts_with(&staging_root) || staging_root.starts_with(&root) {
        return Err(RecordingArtifactStoreError::InvalidConfiguration(
            "finalized and staging roots resolve to overlapping locations".to_string(),
        ));
    }
    Ok(())
}

fn is_normal_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            )
        })
}

fn validate_source_path(source_path: &str) -> Result<PathBuf, RecordingArtifactStoreError> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return Err(RecordingArtifactStoreError::InvalidSourcePath(
            "source path must not be empty".to_string(),
        ));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(RecordingArtifactStoreError::InvalidSourcePath(
            "source path must be absolute for local_fs storage".to_string(),
        ));
    }
    Ok(path)
}

fn relative_artifact_path(
    session_id: Uuid,
    recording_id: Uuid,
    format: SessionRecordingFormat,
) -> PathBuf {
    let extension = match format {
        SessionRecordingFormat::Webm => "webm",
    };
    PathBuf::from(session_id.to_string()).join(format!("{recording_id}.{extension}"))
}

fn should_fallback_to_copy(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::CrossesDevices
        || error.kind() == std::io::ErrorKind::PermissionDenied
        || error.raw_os_error() == Some(30)
}

fn should_ignore_source_cleanup_error(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(30)
}

#[cfg(test)]
mod tests;
