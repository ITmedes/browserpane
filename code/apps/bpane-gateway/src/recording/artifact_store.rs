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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecordingArtifact {
    pub artifact_ref: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RecordingArtifactStoreError {
    #[error("invalid artifact reference: {0}")]
    InvalidReference(String),
    #[error("invalid source path: {0}")]
    InvalidSourcePath(String),
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

    pub fn local_fs(root: PathBuf) -> Self {
        Self::new(Arc::new(LocalFsRecordingArtifactStore { root }))
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
}

#[async_trait]
impl RecordingArtifactStoreBackend for LocalFsRecordingArtifactStore {
    async fn finalize(
        &self,
        request: FinalizeRecordingArtifactRequest,
    ) -> Result<StoredRecordingArtifact, RecordingArtifactStoreError> {
        let source_path = validate_source_path(&request.source_path)?;
        let relative =
            relative_artifact_path(request.session_id, request.recording_id, request.format);
        let destination = self.root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }

        match fs::rename(&source_path, &destination).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
                fs::copy(&source_path, &destination).await?;
                fs::remove_file(&source_path).await?;
            }
            Err(error) => return Err(error.into()),
        }

        Ok(StoredRecordingArtifact {
            artifact_ref: format!("{LOCAL_FS_REF_PREFIX}{}", relative.to_string_lossy()),
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

impl LocalFsRecordingArtifactStore {
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

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn local_fs_store_moves_source_file_into_managed_root() {
        let temp_dir = tempdir().unwrap();
        let source = temp_dir.path().join("source.webm");
        std::fs::write(&source, b"artifact").unwrap();
        let root = temp_dir.path().join("artifacts");
        let store = RecordingArtifactStore::local_fs(root.clone());
        let session_id = Uuid::now_v7();
        let recording_id = Uuid::now_v7();

        let artifact = store
            .finalize(FinalizeRecordingArtifactRequest {
                session_id,
                recording_id,
                format: SessionRecordingFormat::Webm,
                source_path: source.to_string_lossy().to_string(),
            })
            .await
            .unwrap();

        assert!(!source.exists());
        assert_eq!(
            artifact.artifact_ref,
            format!("{LOCAL_FS_REF_PREFIX}{session_id}/{recording_id}.webm")
        );
        let bytes = store.read(&artifact.artifact_ref).await.unwrap();
        assert_eq!(bytes.as_slice(), b"artifact");
        assert!(root
            .join(session_id.to_string())
            .join(format!("{recording_id}.webm"))
            .exists());
    }

    #[tokio::test]
    async fn local_fs_store_rejects_invalid_references() {
        let temp_dir = tempdir().unwrap();
        let store = RecordingArtifactStore::local_fs(temp_dir.path().join("artifacts"));
        let error = store.read("../../../etc/passwd").await.unwrap_err();
        assert!(matches!(
            error,
            RecordingArtifactStoreError::InvalidReference(_)
        ));
    }
}
