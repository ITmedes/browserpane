use std::time::Duration;

use async_trait::async_trait;
use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{
    AttachContainerOptionsBuilder, CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder,
    RemoveVolumeOptionsBuilder, WaitContainerOptionsBuilder,
};
use bollard::{Docker, API_DEFAULT_VERSION};
use futures_util::StreamExt;
use reqwest::Url;
use tokio::io::AsyncWriteExt;

use crate::{ExecutionError, ExecutionErrorCode};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum StorageDockerError {
    NotFound,
    Failed,
}

#[async_trait]
pub(super) trait StorageDockerApi: Send + Sync {
    async fn ping(&self) -> Result<(), StorageDockerError>;
    async fn volume_exists(&self, name: &str) -> Result<bool, StorageDockerError>;
    async fn remove_volume(&self, name: &str) -> Result<(), StorageDockerError>;
    async fn run_helper(
        &self,
        name: String,
        body: ContainerCreateBody,
        input: Option<Vec<u8>>,
        output_limit: usize,
    ) -> Result<Vec<u8>, StorageDockerError>;
}

pub(super) struct BollardStorageDockerApi {
    docker: Docker,
    timeout: Duration,
}

impl BollardStorageDockerApi {
    pub(super) fn connect(docker_api_url: &str, timeout: Duration) -> Result<Self, ExecutionError> {
        let url = Url::parse(docker_api_url).map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        if url.scheme() != "http"
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(url.path(), "" | "/")
            || url.query().is_some()
            || url.fragment().is_some()
            || timeout.is_zero()
            || timeout > Duration::from_secs(300)
        {
            return Err(ExecutionErrorCode::AdapterFailed.into());
        }
        let docker = Docker::connect_with_http(
            docker_api_url,
            timeout.as_secs().max(1),
            API_DEFAULT_VERSION,
        )
        .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        Ok(Self { docker, timeout })
    }
}

#[async_trait]
impl StorageDockerApi for BollardStorageDockerApi {
    async fn ping(&self) -> Result<(), StorageDockerError> {
        self.docker
            .ping()
            .await
            .map(|_| ())
            .map_err(map_bollard_error)
    }

    async fn volume_exists(&self, name: &str) -> Result<bool, StorageDockerError> {
        self.docker.inspect_volume(name).await.map_or_else(
            |error| match map_bollard_error(error) {
                StorageDockerError::NotFound => Ok(false),
                StorageDockerError::Failed => Err(StorageDockerError::Failed),
            },
            |_| Ok(true),
        )
    }

    async fn remove_volume(&self, name: &str) -> Result<(), StorageDockerError> {
        let options = RemoveVolumeOptionsBuilder::default().force(true).build();
        self.docker
            .remove_volume(name, Some(options))
            .await
            .map_err(map_bollard_error)
    }

    async fn run_helper(
        &self,
        name: String,
        body: ContainerCreateBody,
        input: Option<Vec<u8>>,
        output_limit: usize,
    ) -> Result<Vec<u8>, StorageDockerError> {
        let docker = self.docker.clone();
        let timeout = self.timeout;
        let task = tokio::spawn(async move {
            let operation = tokio::time::timeout(
                timeout,
                execute_helper(&docker, &name, body, input, output_limit),
            )
            .await
            .map_err(|_| StorageDockerError::Failed)
            .and_then(|result| result);
            let remove = RemoveContainerOptionsBuilder::default()
                .force(true)
                .v(false)
                .build();
            let cleanup = docker.remove_container(&name, Some(remove)).await;
            match (operation, cleanup) {
                (
                    Ok(output),
                    Ok(())
                    | Err(BollardError::DockerResponseServerError {
                        status_code: 404, ..
                    }),
                ) => Ok(output),
                (Ok(_), Err(_)) | (Err(_), _) => Err(StorageDockerError::Failed),
            }
        });
        task.await.map_err(|_| StorageDockerError::Failed)?
    }
}

async fn execute_helper(
    docker: &Docker,
    name: &str,
    body: ContainerCreateBody,
    input: Option<Vec<u8>>,
    output_limit: usize,
) -> Result<Vec<u8>, StorageDockerError> {
    let remove = RemoveContainerOptionsBuilder::default()
        .force(true)
        .v(false)
        .build();
    match docker.remove_container(name, Some(remove)).await {
        Ok(())
        | Err(BollardError::DockerResponseServerError {
            status_code: 404, ..
        }) => {}
        Err(_) => return Err(StorageDockerError::Failed),
    }
    let create = CreateContainerOptionsBuilder::default().name(name).build();
    docker
        .create_container(Some(create), body)
        .await
        .map_err(map_bollard_error)?;
    let attach = AttachContainerOptionsBuilder::default()
        .stdin(input.is_some())
        .stdout(true)
        .stderr(true)
        .stream(true)
        .logs(true)
        .build();
    let bollard::container::AttachContainerResults {
        mut output,
        input: mut container_input,
    } = docker
        .attach_container(name, Some(attach))
        .await
        .map_err(map_bollard_error)?;
    docker
        .start_container(name, None)
        .await
        .map_err(map_bollard_error)?;

    let writer = async move {
        if let Some(bytes) = input {
            container_input
                .write_all(&bytes)
                .await
                .map_err(|_| StorageDockerError::Failed)?;
        }
        container_input
            .shutdown()
            .await
            .map_err(|_| StorageDockerError::Failed)
    };
    let reader = async move {
        let mut stdout = Vec::new();
        let mut stderr_bytes = 0_usize;
        while let Some(item) = output.next().await {
            match item.map_err(map_bollard_error)? {
                LogOutput::StdOut { message } | LogOutput::Console { message } => {
                    if stdout.len().saturating_add(message.len()) > output_limit {
                        return Err(StorageDockerError::Failed);
                    }
                    stdout.extend_from_slice(&message);
                }
                LogOutput::StdErr { message } => {
                    stderr_bytes = stderr_bytes.saturating_add(message.len());
                    if stderr_bytes > 65_536 {
                        return Err(StorageDockerError::Failed);
                    }
                }
                LogOutput::StdIn { .. } => {}
            }
        }
        Ok(stdout)
    };
    let waiter = async move {
        let options = WaitContainerOptionsBuilder::default()
            .condition("not-running")
            .build();
        let result = docker
            .wait_container(name, Some(options))
            .next()
            .await
            .ok_or(StorageDockerError::Failed)?;
        let response = result.map_err(map_bollard_error)?;
        require_successful_exit(response.status_code)
    };
    let (_, stdout, _) = tokio::try_join!(writer, reader, waiter)?;
    Ok(stdout)
}

fn require_successful_exit(status_code: i64) -> Result<(), StorageDockerError> {
    if status_code == 0 {
        Ok(())
    } else {
        Err(StorageDockerError::Failed)
    }
}

fn map_bollard_error(error: BollardError) -> StorageDockerError {
    if matches!(
        error,
        BollardError::DockerResponseServerError {
            status_code: 404,
            ..
        }
    ) {
        StorageDockerError::NotFound
    } else {
        StorageDockerError::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_successful_helper_exit_status() {
        assert_eq!(require_successful_exit(0), Ok(()));
        assert_eq!(require_successful_exit(64), Err(StorageDockerError::Failed));
        assert_eq!(
            require_successful_exit(137),
            Err(StorageDockerError::Failed)
        );
    }
}
