mod postgres;
mod recordings;
mod resources;
mod sessions;
mod workflows;

use super::support::principal;
use super::*;

const POSTGRES_URL_ENV: &str = "BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL";

#[tokio::test]
async fn session_store_contract_in_memory() {
    run_contracts(&SessionStore::in_memory_with_config(
        contract_runtime_profile(),
    ))
    .await
    .expect("in-memory session-store contract should pass");
}

#[tokio::test]
#[ignore = "requires BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL"]
async fn session_store_contract_postgres() {
    let database_url = std::env::var(POSTGRES_URL_ENV)
        .unwrap_or_else(|_| panic!("{POSTGRES_URL_ENV} must be configured"));
    let fixture = postgres::PostgresContractFixture::create(&database_url)
        .await
        .expect("Postgres contract fixture should initialize");
    let contract_result = run_contracts(fixture.store()).await;
    let cleanup_result = fixture.cleanup().await;

    contract_result.expect("Postgres session-store contract should pass");
    cleanup_result.expect("Postgres contract fixture should clean up");
}

async fn run_contracts(store: &SessionStore) -> anyhow::Result<()> {
    resources::run_resource_contracts(store).await?;
    sessions::run_session_contracts(store).await?;
    workflows::run_workflow_contracts(store).await?;
    recordings::run_recording_contracts(store).await
}

fn contract_runtime_profile() -> SessionManagerProfile {
    SessionManagerProfile {
        runtime_binding: "contract_runtime_pool".to_string(),
        compatibility_mode: "contract_test".to_string(),
        max_runtime_sessions: 32,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    }
}
