use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::fs;
use uuid::Uuid;

const LOCAL_FS_REF_PREFIX: &str = "local_fs:";

#[derive(Debug, Clone)]
pub struct StoreWorkspaceFileRequest {
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWorkspaceFileArtifact {
    pub artifact_ref: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceFileStoreError {
    #[error("invalid workspace file reference: {0}")]
    InvalidReference(String),
    #[error("invalid workspace file name: {0}")]
    InvalidFileName(String),
    #[error("workspace file backend failed: {0}")]
    Backend(#[from] std::io::Error),
}

impl WorkspaceFileStoreError {
    pub fn io_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            Self::Backend(error) => Some(error.kind()),
            _ => None,
        }
    }
}

#[async_trait]
pub trait WorkspaceFileStoreBackend: Send + Sync {
    async fn write(
        &self,
        request: StoreWorkspaceFileRequest,
    ) -> Result<StoredWorkspaceFileArtifact, WorkspaceFileStoreError>;

    async fn read(&self, artifact_ref: &str) -> Result<Vec<u8>, WorkspaceFileStoreError>;

    async fn delete(&self, artifact_ref: &str) -> Result<(), WorkspaceFileStoreError>;
}

#[derive(Clone)]
pub struct WorkspaceFileStore {
    backend: Arc<dyn WorkspaceFileStoreBackend>,
}

impl WorkspaceFileStore {
    pub fn new(backend: Arc<dyn WorkspaceFileStoreBackend>) -> Self {
        Self { backend }
    }

    pub fn local_fs(root: PathBuf) -> Self {
        Self::new(Arc::new(LocalFsWorkspaceFileStore { root }))
    }

    pub async fn write(
        &self,
        request: StoreWorkspaceFileRequest,
    ) -> Result<StoredWorkspaceFileArtifact, WorkspaceFileStoreError> {
        self.backend.write(request).await
    }

    pub async fn read(&self, artifact_ref: &str) -> Result<Vec<u8>, WorkspaceFileStoreError> {
        self.backend.read(artifact_ref).await
    }

    pub async fn delete(&self, artifact_ref: &str) -> Result<(), WorkspaceFileStoreError> {
        self.backend.delete(artifact_ref).await
    }
}

#[derive(Debug)]
pub struct LocalFsWorkspaceFileStore {
    root: PathBuf,
}

#[async_trait]
impl WorkspaceFileStoreBackend for LocalFsWorkspaceFileStore {
    async fn write(
        &self,
        request: StoreWorkspaceFileRequest,
    ) -> Result<StoredWorkspaceFileArtifact, WorkspaceFileStoreError> {
        let relative = relative_workspace_file_path(
            request.workspace_id,
            request.file_id,
            &request.file_name,
        )?;
        let destination = self.root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&destination, request.bytes).await?;
        Ok(StoredWorkspaceFileArtifact {
            artifact_ref: format!("{LOCAL_FS_REF_PREFIX}{}", relative.to_string_lossy()),
        })
    }

    async fn read(&self, artifact_ref: &str) -> Result<Vec<u8>, WorkspaceFileStoreError> {
        let path = self.resolve_path(artifact_ref)?;
        Ok(fs::read(path).await?)
    }

    async fn delete(&self, artifact_ref: &str) -> Result<(), WorkspaceFileStoreError> {
        let path = self.resolve_path(artifact_ref)?;
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl LocalFsWorkspaceFileStore {
    fn resolve_path(&self, artifact_ref: &str) -> Result<PathBuf, WorkspaceFileStoreError> {
        let relative = artifact_ref
            .strip_prefix(LOCAL_FS_REF_PREFIX)
            .ok_or_else(|| WorkspaceFileStoreError::InvalidReference(artifact_ref.to_string()))?;
        let path = Path::new(relative);
        if path.as_os_str().is_empty() {
            return Err(WorkspaceFileStoreError::InvalidReference(
                "artifact reference path must not be empty".to_string(),
            ));
        }
        for component in path.components() {
            match component {
                Component::Normal(_) => {}
                _ => {
                    return Err(WorkspaceFileStoreError::InvalidReference(
                        artifact_ref.to_string(),
                    ));
                }
            }
        }
        Ok(self.root.join(path))
    }
}

fn relative_workspace_file_path(
    workspace_id: Uuid,
    file_id: Uuid,
    file_name: &str,
) -> Result<PathBuf, WorkspaceFileStoreError> {
    let sanitized = sanitize_file_name(file_name)?;
    Ok(PathBuf::from(workspace_id.to_string()).join(format!("{file_id}-{sanitized}")))
}

fn sanitize_file_name(file_name: &str) -> Result<String, WorkspaceFileStoreError> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return Err(WorkspaceFileStoreError::InvalidFileName(
            "file name must not be empty".to_string(),
        ));
    }
    let path = Path::new(trimmed);
    let Some(name) = path.file_name() else {
        return Err(WorkspaceFileStoreError::InvalidFileName(
            "file name must not be a path".to_string(),
        ));
    };
    let sanitized = name.to_string_lossy();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        return Err(WorkspaceFileStoreError::InvalidFileName(
            "file name must contain a normal basename".to_string(),
        ));
    }
    Ok(sanitized.into_owned())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn local_fs_store_writes_reads_and_deletes_workspace_files() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().join("workspace-files");
        let store = WorkspaceFileStore::local_fs(root.clone());
        let workspace_id = Uuid::now_v7();
        let file_id = Uuid::now_v7();

        let stored = store
            .write(StoreWorkspaceFileRequest {
                workspace_id,
                file_id,
                file_name: "report.csv".to_string(),
                bytes: b"alpha,beta\n1,2\n".to_vec(),
            })
            .await
            .unwrap();

        assert_eq!(
            stored.artifact_ref,
            format!("{LOCAL_FS_REF_PREFIX}{workspace_id}/{file_id}-report.csv")
        );
        let bytes = store.read(&stored.artifact_ref).await.unwrap();
        assert_eq!(bytes, b"alpha,beta\n1,2\n".to_vec());

        store.delete(&stored.artifact_ref).await.unwrap();
        let error = store.read(&stored.artifact_ref).await.unwrap_err();
        assert_eq!(error.io_kind(), Some(std::io::ErrorKind::NotFound));
    }

    #[tokio::test]
    async fn local_fs_store_rejects_invalid_references() {
        let temp_dir = tempdir().unwrap();
        let store = WorkspaceFileStore::local_fs(temp_dir.path().join("workspace-files"));
        let error = store.read("../../../etc/passwd").await.unwrap_err();
        assert!(matches!(
            error,
            WorkspaceFileStoreError::InvalidReference(_)
        ));
    }
}
