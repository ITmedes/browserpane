use std::time::Duration;

use async_trait::async_trait;
use bollard::errors::Error as BollardError;
use bollard::models::{ContainerCreateBody, ContainerStateStatusEnum};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder, StopContainerOptionsBuilder,
    UploadToContainerOptionsBuilder,
};
use bollard::{Docker, API_DEFAULT_VERSION};
use bytes::Bytes;
use reqwest::Url;

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
    async fn upload_file(
        &self,
        _name: &str,
        _directory: &str,
        _file_name: &str,
        _contents: Vec<u8>,
    ) -> Result<(), DockerBackendError> {
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

    async fn upload_file(
        &self,
        name: &str,
        directory: &str,
        file_name: &str,
        contents: Vec<u8>,
    ) -> Result<(), DockerBackendError> {
        let options = UploadToContainerOptionsBuilder::default()
            .path(directory)
            .build();
        let archive = file_archive_chunks(file_name, contents)?;
        self.docker
            .upload_to_container(
                name,
                Some(options),
                bollard::body_stream(futures_util::stream::iter(archive)),
            )
            .await
            .map_err(map_bollard_error)
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

fn file_archive_chunks(
    file_name: &str,
    contents: Vec<u8>,
) -> Result<Vec<Bytes>, DockerBackendError> {
    if file_name.is_empty()
        || file_name.len() > 128
        || matches!(file_name, "." | "..")
        || !file_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || contents.is_empty()
        || contents.len() > 64 * 1024
    {
        return Err(DockerBackendError::Failed);
    }
    let contents_len = u64::try_from(contents.len()).map_err(|_| DockerBackendError::Failed)?;
    let mut header = tar::Header::new_gnu();
    header
        .set_path(file_name)
        .map_err(|_| DockerBackendError::Failed)?;
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o400);
    header.set_uid(0);
    header.set_gid(0);
    header.set_size(contents_len);
    header.set_cksum();
    let padding = (512 - contents.len() % 512) % 512;
    Ok(vec![
        Bytes::copy_from_slice(header.as_bytes()),
        Bytes::from(contents),
        Bytes::from(vec![0_u8; padding + 1024]),
    ])
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

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn file_archive_uses_a_private_regular_file() {
        let expected = br#"{"token":"secret"}"#.to_vec();
        let archive = file_archive_chunks("worker.json", expected.clone())
            .unwrap()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut archive = tar::Archive::new(archive.as_slice());
        let mut entries = archive.entries().unwrap();
        let mut entry = entries.next().unwrap().unwrap();
        assert_eq!(
            entry.path().unwrap().as_ref(),
            std::path::Path::new("worker.json")
        );
        assert_eq!(entry.header().mode().unwrap(), 0o400);
        let mut actual = Vec::new();
        entry.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
        assert!(entries.next().is_none());
    }

    #[test]
    fn file_archive_rejects_paths_and_unbounded_content() {
        assert_eq!(
            file_archive_chunks("../worker.json", vec![1]).unwrap_err(),
            DockerBackendError::Failed
        );
        assert_eq!(
            file_archive_chunks("..", vec![1]).unwrap_err(),
            DockerBackendError::Failed
        );
        assert_eq!(
            file_archive_chunks("worker.json", vec![0; 64 * 1024 + 1]).unwrap_err(),
            DockerBackendError::Failed
        );
    }
}
