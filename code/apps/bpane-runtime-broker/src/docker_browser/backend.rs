use std::time::Duration;

use async_trait::async_trait;
use bollard::errors::Error as BollardError;
use bollard::models::{ContainerCreateBody, ContainerStateStatusEnum};
use bollard::query_parameters::{
    AttachContainerOptionsBuilder, CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder,
    StopContainerOptionsBuilder,
};
use bollard::{Docker, API_DEFAULT_VERSION};
use reqwest::Url;
use tokio::io::AsyncWriteExt;

use crate::{ExecutionError, ExecutionErrorCode};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DockerBackendError {
    NotFound,
    Failed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DockerContainerState {
    Running,
    Exited { exit_code: Option<i32> },
}

#[async_trait]
pub(crate) trait DockerContainerApi: Send + Sync {
    async fn ping(&self) -> Result<(), DockerBackendError>;
    async fn create(&self, name: &str, body: ContainerCreateBody)
        -> Result<(), DockerBackendError>;
    async fn start(&self, name: &str) -> Result<(), DockerBackendError>;
    async fn send_stdin(&self, _name: &str, _contents: Vec<u8>) -> Result<(), DockerBackendError> {
        Err(DockerBackendError::Failed)
    }
    async fn inspect(&self, name: &str) -> Result<DockerContainerState, DockerBackendError>;
    async fn stop(&self, name: &str) -> Result<(), DockerBackendError>;
    async fn remove(&self, name: &str) -> Result<(), DockerBackendError>;
}

pub(crate) struct BollardDockerContainerApi {
    docker: Docker,
}

impl BollardDockerContainerApi {
    pub(crate) fn connect(docker_api_url: &str, timeout: Duration) -> Result<Self, ExecutionError> {
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
        let timeout_secs = timeout.as_secs().max(1);
        let docker = Docker::connect_with_http(docker_api_url, timeout_secs, API_DEFAULT_VERSION)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        Ok(Self { docker })
    }
}

#[async_trait]
impl DockerContainerApi for BollardDockerContainerApi {
    async fn ping(&self) -> Result<(), DockerBackendError> {
        self.docker
            .ping()
            .await
            .map(|_| ())
            .map_err(map_bollard_error)
    }

    async fn create(
        &self,
        name: &str,
        body: ContainerCreateBody,
    ) -> Result<(), DockerBackendError> {
        let options = CreateContainerOptionsBuilder::default().name(name).build();
        self.docker
            .create_container(Some(options), body)
            .await
            .map(|_| ())
            .map_err(map_bollard_error)
    }

    async fn start(&self, name: &str) -> Result<(), DockerBackendError> {
        self.docker
            .start_container(name, None)
            .await
            .map_err(map_bollard_error)
    }

    async fn send_stdin(
        &self,
        name: &str,
        mut contents: Vec<u8>,
    ) -> Result<(), DockerBackendError> {
        if contents.is_empty() || contents.len() > 64 * 1024 || contents.contains(&b'\n') {
            return Err(DockerBackendError::Failed);
        }
        contents.push(b'\n');
        let options = AttachContainerOptionsBuilder::default()
            .stdin(true)
            .stream(true)
            .build();
        let mut attachment = self
            .docker
            .attach_container(name, Some(options))
            .await
            .map_err(map_bollard_error)?;
        attachment
            .input
            .write_all(&contents)
            .await
            .map_err(|_| DockerBackendError::Failed)?;
        attachment
            .input
            .shutdown()
            .await
            .map_err(|_| DockerBackendError::Failed)
    }

    async fn inspect(&self, name: &str) -> Result<DockerContainerState, DockerBackendError> {
        let response = self
            .docker
            .inspect_container(name, None)
            .await
            .map_err(map_bollard_error)?;
        let state = response.state.ok_or(DockerBackendError::Failed)?;
        match state.status {
            Some(ContainerStateStatusEnum::EXITED | ContainerStateStatusEnum::DEAD) => {
                Ok(DockerContainerState::Exited {
                    exit_code: state.exit_code.and_then(|value| i32::try_from(value).ok()),
                })
            }
            Some(
                ContainerStateStatusEnum::CREATED
                | ContainerStateStatusEnum::RUNNING
                | ContainerStateStatusEnum::PAUSED
                | ContainerStateStatusEnum::RESTARTING
                | ContainerStateStatusEnum::REMOVING
                | ContainerStateStatusEnum::STOPPING,
            ) => Ok(DockerContainerState::Running),
            Some(ContainerStateStatusEnum::EMPTY) | None => Err(DockerBackendError::Failed),
        }
    }

    async fn stop(&self, name: &str) -> Result<(), DockerBackendError> {
        let options = StopContainerOptionsBuilder::default().t(10).build();
        self.docker
            .stop_container(name, Some(options))
            .await
            .map_err(map_bollard_error)
    }

    async fn remove(&self, name: &str) -> Result<(), DockerBackendError> {
        let options = RemoveContainerOptionsBuilder::default()
            .force(true)
            .v(false)
            .build();
        self.docker
            .remove_container(name, Some(options))
            .await
            .map_err(map_bollard_error)
    }
}

fn map_bollard_error(error: BollardError) -> DockerBackendError {
    if matches!(
        error,
        BollardError::DockerResponseServerError {
            status_code: 404,
            ..
        }
    ) {
        DockerBackendError::NotFound
    } else {
        DockerBackendError::Failed
    }
}
