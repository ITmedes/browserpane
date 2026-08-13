//! Authenticated, bounded runtime-operation broker for BrowserPane.
//!
//! The broker accepts only the typed product operations defined by
//! `bpane-runtime-contract`. Docker-specific materialization remains behind an
//! executor boundary and is intentionally absent from the public HTTP API.

#![forbid(unsafe_code)]

mod api;
mod auth;
mod browser_adapter_config;
mod config;
mod docker_browser;
mod docker_storage;
mod docker_workers;
mod executor;
mod ledger;
mod storage_adapter_config;
mod worker_adapter_config;

pub use api::{build_router, BrokerApiErrorCode, BrokerApiSettings, BrokerState};
pub use auth::{
    AuthenticationError, AuthenticationErrorCode, BrokerAuthenticator, OidcAuthenticatorConfig,
    OidcBrokerAuthenticator, ServicePrincipal,
};
pub use browser_adapter_config::{BrowserAdapterSettings, RuntimeExecutorMode};
pub use config::BrokerConfig;
pub use docker_browser::{
    BrowserRuntimeDockerAdapter, BrowserRuntimeDockerConfig, BrowserRuntimeExtensionConfig,
};
pub use docker_storage::{StorageRuntimeDockerAdapter, StorageRuntimeDockerConfig};
pub use docker_workers::{
    RecordingWorkerDockerConfig, WorkerOidcConfig, WorkerRuntimeDockerAdapter,
    WorkerRuntimeDockerConfig, WorkflowWorkerDockerConfig,
};
pub use executor::{
    ExecutionError, ExecutionErrorCode, RejectingRuntimeExecutor, RuntimeOperationExecutor,
    StorageExecutionOutput,
};
pub use ledger::{LedgerConfig, OperationLedger};
pub use storage_adapter_config::StorageAdapterSettings;
pub use worker_adapter_config::WorkerAdapterSettings;
