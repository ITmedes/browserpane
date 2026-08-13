//! Bounded OAuth2 client for the BrowserPane runtime broker.
//!
//! This crate owns service-token acquisition and typed broker HTTP transport.
//! It does not expose raw response bodies, URLs containing credentials, or
//! backend-specific runtime models to callers.

#![forbid(unsafe_code)]

mod client;
mod error;
mod token;

pub use client::{
    HttpRuntimeBrokerClient, RuntimeBrokerClient, RuntimeBrokerClientConfig,
    RuntimeStorageOperationResponse,
};
pub use error::{RuntimeBrokerClientError, RuntimeBrokerClientErrorCode};
pub use token::{
    AccessTokenProvider, Oauth2ClientCredentialsConfig, Oauth2ClientCredentialsProvider,
};
