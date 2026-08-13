//! Authenticated, bounded runtime-operation broker for BrowserPane.
//!
//! The broker accepts only the typed product operations defined by
//! `bpane-runtime-contract`. Docker-specific materialization remains behind an
//! executor boundary and is intentionally absent from the public HTTP API.

#![forbid(unsafe_code)]

mod api;
mod auth;
mod config;
mod docker_browser;
mod executor;
mod ledger;

pub use api::{build_router, BrokerApiErrorCode, BrokerApiSettings, BrokerState};
pub use auth::{
    AuthenticationError, AuthenticationErrorCode, BrokerAuthenticator, OidcAuthenticatorConfig,
    OidcBrokerAuthenticator, ServicePrincipal,
};
pub use config::BrokerConfig;
pub use docker_browser::{BrowserRuntimeDockerAdapter, BrowserRuntimeDockerConfig};
pub use executor::{
    ExecutionError, ExecutionErrorCode, RejectingRuntimeExecutor, RuntimeOperationExecutor,
};
pub use ledger::{LedgerConfig, OperationLedger};
