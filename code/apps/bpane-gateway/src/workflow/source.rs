use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

mod archive;
mod git;
mod validation;

pub use validation::validate_workflow_source_entrypoint;

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
}
