use std::sync::Arc;

use bpane_runtime_broker::{
    build_router, BrokerConfig, BrokerState, OidcBrokerAuthenticator, OperationLedger,
};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("bpane_runtime_broker=info,tower_http=info")),
        )
        .init();

    let config = BrokerConfig::parse();
    let (api_settings, ledger_config, oidc_config) = config.validated()?;
    let executor = config.browser_adapter_settings().build_executor()?;
    let authenticator = Arc::new(OidcBrokerAuthenticator::new(oidc_config).await?);
    let state = BrokerState::new(
        authenticator,
        executor,
        Arc::new(OperationLedger::new(ledger_config)),
        api_settings,
    );
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(listen = %config.listen, "runtime broker listening");
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
