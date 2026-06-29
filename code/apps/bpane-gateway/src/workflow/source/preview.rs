use std::path::Path;

use tokio::io::AsyncReadExt;

use super::archive::TemporaryWorkflowSourceDir;
use super::validation::{join_validated_relative_path, validate_workflow_source_entrypoint};
use super::{WorkflowSource, WorkflowSourceError, WorkflowSourcePreview, WorkflowSourceResolver};

impl WorkflowSourceResolver {
    pub async fn materialize_entrypoint_preview(
        &self,
        source: &WorkflowSource,
        entrypoint: &str,
        max_bytes: usize,
    ) -> Result<WorkflowSourcePreview, WorkflowSourceError> {
        if max_bytes == 0 {
            return Err(WorkflowSourceError::Invalid(
                "workflow source preview max_bytes must be greater than zero".to_string(),
            ));
        }
        validate_workflow_source_entrypoint(Some(source), entrypoint)?;
        let resolved_source = self.resolve(Some(source.clone())).await?.ok_or_else(|| {
            WorkflowSourceError::Invalid("workflow source is required".to_string())
        })?;
        match resolved_source {
            WorkflowSource::Git(git_source) => {
                let checkout_dir = TemporaryWorkflowSourceDir::new()?;
                self.clone_and_checkout_git_source(&git_source, checkout_dir.path())
                    .await?;
                let entrypoint_path =
                    join_validated_relative_path(checkout_dir.path(), entrypoint)?;
                if !entrypoint_path.is_file() {
                    return Err(WorkflowSourceError::Materialize(format!(
                        "workflow entrypoint {entrypoint} was not found at commit {}",
                        git_source.resolved_commit.as_deref().unwrap_or("unknown"),
                    )));
                }
                let (content, byte_count, truncated) =
                    read_text_preview(&entrypoint_path, max_bytes).await?;
                Ok(WorkflowSourcePreview {
                    source: WorkflowSource::Git(git_source),
                    entrypoint: entrypoint.to_string(),
                    media_type: media_type_for_entrypoint(entrypoint).to_string(),
                    language: language_for_entrypoint(entrypoint).to_string(),
                    content,
                    byte_count,
                    truncated,
                })
            }
        }
    }
}

async fn read_text_preview(
    path: &Path,
    max_bytes: usize,
) -> Result<(String, usize, bool), WorkflowSourceError> {
    let file = tokio::fs::File::open(path).await.map_err(|error| {
        WorkflowSourceError::Materialize(format!(
            "failed to open workflow entrypoint {}: {error}",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(max_bytes.saturating_add(1));
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            WorkflowSourceError::Materialize(format!(
                "failed to read workflow entrypoint {}: {error}",
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
            "workflow entrypoint {} is not valid UTF-8: {error}",
            path.display()
        ))
    })?;
    Ok((content, byte_count, truncated))
}

fn language_for_entrypoint(entrypoint: &str) -> &'static str {
    match Path::new(entrypoint)
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

fn media_type_for_entrypoint(entrypoint: &str) -> &'static str {
    match Path::new(entrypoint)
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
