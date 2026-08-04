mod postgres;

use anyhow::{ensure, Context};

use super::support::principal;
use super::*;

const POSTGRES_URL_ENV: &str = "BPANE_SESSION_STORE_CONTRACT_POSTGRES_URL";

#[tokio::test]
async fn session_store_contract_in_memory() {
    run_session_visibility_contract(&SessionStore::in_memory())
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
    let contract_result = run_session_visibility_contract(fixture.store()).await;
    let cleanup_result = fixture.cleanup().await;

    contract_result.expect("Postgres session-store contract should pass");
    cleanup_result.expect("Postgres contract fixture should clean up");
}

async fn run_session_visibility_contract(store: &SessionStore) -> anyhow::Result<()> {
    let owner = principal(&format!("contract-owner-{}", Uuid::now_v7()));
    let other_owner = principal(&format!("contract-other-{}", Uuid::now_v7()));
    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                labels: HashMap::from([("contract".to_string(), "visibility".to_string())]),
                ..CreateSessionRequest::default()
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .context("create contract session")?;

    let visible = store
        .list_sessions_for_owner(&owner)
        .await
        .context("list owner sessions")?;
    ensure!(
        visible.iter().any(|session| session.id == created.id),
        "owner session catalog omitted the created session"
    );
    ensure!(
        store
            .get_session_for_owner(&other_owner, created.id)
            .await
            .context("read session as another owner")?
            .is_none(),
        "session was visible to another owner"
    );
    Ok(())
}
