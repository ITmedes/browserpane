use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

const DEFAULT_WORKFLOW_SOURCE_MAX_FILES: usize = 1024;
const DEFAULT_WORKFLOW_SOURCE_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_WORKFLOW_SOURCE_MAX_TOTAL_BYTES: u64 = 50 * 1024 * 1024;

mod archive;
mod git;
mod preview;
mod validation;

#[cfg(test)]
mod tests;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSourcePreview {
    pub source: WorkflowSource,
    pub entrypoint: String,
    pub path: String,
    pub media_type: String,
    pub language: String,
    pub content: String,
    pub byte_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSourceFile {
    pub path: String,
    pub byte_count: u64,
    pub media_type: String,
    pub language: String,
    pub entrypoint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSourceFileListing {
    pub source: WorkflowSource,
    pub files: Vec<WorkflowSourceFile>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowSourceError {
    #[error("invalid workflow source: {0}")]
    Invalid(String),
    #[error("failed to resolve workflow source: {0}")]
    Resolve(String),
    #[error("failed to access workflow source repository: {0}")]
    RepositoryAccess(String),
    #[error("failed to materialize workflow source: {0}")]
    Materialize(String),
    #[error("failed to create workflow source snapshot: {0}")]
    Snapshot(String),
    #[error("workflow source infrastructure unavailable: {0}")]
    Infrastructure(String),
}

#[derive(Debug, Clone)]
pub struct WorkflowSourceResolver {
    git_bin: PathBuf,
    resolve_timeout: Duration,
    materialize_timeout: Duration,
    source_policy: WorkflowSourcePolicy,
}

impl WorkflowSourceResolver {
    pub fn with_policy(git_bin: PathBuf, source_policy: WorkflowSourcePolicy) -> Self {
        Self {
            git_bin,
            resolve_timeout: Duration::from_secs(15),
            materialize_timeout: Duration::from_secs(60),
            source_policy,
        }
    }

    pub fn validate_entrypoint(
        &self,
        source: Option<&WorkflowSource>,
        entrypoint: &str,
    ) -> Result<(), WorkflowSourceError> {
        validation::validate_workflow_source_entrypoint_with_policy(
            &self.source_policy,
            source,
            entrypoint,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSourcePolicy {
    trusted_local_roots: Vec<PathBuf>,
    collection_limits: WorkflowSourceCollectionLimits,
}

impl WorkflowSourcePolicy {
    pub fn with_trusted_local_roots<I>(mut self, trusted_local_roots: I) -> Self
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.trusted_local_roots = trusted_local_roots.into_iter().collect();
        self
    }

    pub fn with_collection_limits(
        mut self,
        max_files: usize,
        max_file_bytes: u64,
        max_total_bytes: u64,
    ) -> Self {
        self.collection_limits = WorkflowSourceCollectionLimits {
            max_files,
            max_file_bytes,
            max_total_bytes,
        };
        self
    }

    pub(super) fn trusted_local_roots(&self) -> &[PathBuf] {
        &self.trusted_local_roots
    }

    pub(super) fn collection_limits(&self) -> WorkflowSourceCollectionLimits {
        self.collection_limits
    }
}

impl Default for WorkflowSourcePolicy {
    fn default() -> Self {
        Self {
            trusted_local_roots: Vec::new(),
            collection_limits: WorkflowSourceCollectionLimits {
                max_files: DEFAULT_WORKFLOW_SOURCE_MAX_FILES,
                max_file_bytes: DEFAULT_WORKFLOW_SOURCE_MAX_FILE_BYTES,
                max_total_bytes: DEFAULT_WORKFLOW_SOURCE_MAX_TOTAL_BYTES,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WorkflowSourceCollectionLimits {
    pub(super) max_files: usize,
    pub(super) max_file_bytes: u64,
    pub(super) max_total_bytes: u64,
}
