use std::time::Duration;

use async_trait::async_trait;
use bollard::errors::Error as BollardError;
use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use bollard::{Docker, API_DEFAULT_VERSION};
use reqwest::Url;

use crate::{ExecutionError, ExecutionErrorCode};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum DockerBackendError {
    NotFound,
    Failed,
}

#[async_trait]
pub(super) trait DockerContainerApi: Send + Sync {
    async fn create(&self, name: &str, body: ContainerCreateBody)
        -> Result<(), DockerBackendError>;
    async fn start(&self, name: &str) -> Result<(), DockerBackendError>;
    async fn inspect(&self, name: &str) -> Result<(), DockerBackendError>;
    async fn stop(&self, name: &str) -> Result<(), DockerBackendError>;
    async fn remove(&self, name: &str) -> Result<(), DockerBackendError>;
}

pub(super) struct BollardDockerContainerApi {
    docker: Docker,
}

impl BollardDockerContainerApi {
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
        let timeout_secs = timeout.as_secs().max(1);
        let docker = Docker::connect_with_http(docker_api_url, timeout_secs, API_DEFAULT_VERSION)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        Ok(Self { docker })
    }
}

#[async_trait]
impl DockerContainerApi for BollardDockerContainerApi {
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

    async fn inspect(&self, name: &str) -> Result<(), DockerBackendError> {
        self.docker
            .inspect_container(name, None)
            .await
            .map(|_| ())
            .map_err(map_bollard_error)
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
