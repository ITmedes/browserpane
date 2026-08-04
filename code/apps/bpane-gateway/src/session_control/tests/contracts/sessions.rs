use anyhow::{ensure, Context};

use super::*;

pub(super) async fn run_session_contracts(store: &SessionStore) -> anyhow::Result<()> {
    let suffix = Uuid::now_v7().simple().to_string();
    let owner = principal(&format!("session-owner-{suffix}"));
    let other_owner = principal(&format!("session-other-{suffix}"));
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: format!("session-project-{suffix}"),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: Some(2),
                    ..ProjectQuotas::default()
                },
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .context("create session contract project")?;
    let created = store
        .create_session(
            &owner,
            session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .context("create contract session")?;
    ensure!(
        created.state == SessionLifecycleState::Ready,
        "new session was not ready"
    );
    ensure!(
        created.project_id == Some(project.id),
        "session lost its project binding"
    );

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
    ensure!(
        store
            .stop_session_for_owner(&other_owner, created.id)
            .await
            .context("stop session as another owner")?
            .is_none(),
        "another owner stopped the session"
    );
    ensure!(
        store
            .release_session_runtime_for_owner(&other_owner, created.id)
            .await
            .context("release session as another owner")?
            .is_none(),
        "another owner released the session runtime"
    );

    let foreign_project_error = expected_store_error(
        store
            .create_session(
                &other_owner,
                session_request(project.id),
                SessionOwnerMode::Collaborative,
            )
            .await,
        "foreign project session should be rejected",
    )?;
    ensure!(
        matches!(foreign_project_error, SessionStoreError::NotFound(_)),
        "foreign project session returned the wrong error class"
    );

    let active = store
        .mark_session_active(created.id)
        .await
        .context("mark contract session active")?
        .context("contract session disappeared before active transition")?;
    ensure!(
        active.state == SessionLifecycleState::Active,
        "session was not active"
    );
    let active_again = store
        .mark_session_active(created.id)
        .await
        .context("repeat active transition")?
        .context("contract session disappeared during repeated active transition")?;
    ensure!(
        active_again.updated_at == active.updated_at,
        "repeated active transition was not idempotent"
    );

    let recovered = store
        .mark_session_ready_after_runtime_loss(created.id)
        .await
        .context("restore contract session after runtime loss")?
        .context("contract session disappeared during runtime-loss recovery")?;
    ensure!(
        recovered.state == SessionLifecycleState::Ready,
        "runtime-loss recovery did not restore the session to ready"
    );
    ensure!(
        recovered.project_id == created.project_id
            && recovered.admission == created.admission
            && recovered.capabilities == created.capabilities,
        "runtime-loss recovery did not preserve the session contract"
    );
    let active = store
        .mark_session_active(created.id)
        .await
        .context("reactivate contract session after runtime loss")?
        .context("contract session disappeared after runtime-loss recovery")?;
    ensure!(
        active.state == SessionLifecycleState::Active,
        "recovered session was not active"
    );

    let idle = store
        .mark_session_idle(created.id)
        .await
        .context("mark contract session idle")?
        .context("contract session disappeared before idle transition")?;
    ensure!(
        idle.state == SessionLifecycleState::Idle,
        "session was not idle"
    );
    let idle_again = store
        .mark_session_idle(created.id)
        .await
        .context("repeat idle transition")?
        .context("contract session disappeared during repeated idle transition")?;
    ensure!(
        idle_again.updated_at == idle.updated_at,
        "repeated idle transition was not idempotent"
    );

    let stopped = store
        .stop_session_if_idle(created.id)
        .await
        .context("stop idle contract session")?
        .context("contract session disappeared before stop")?;
    ensure!(
        stopped.state == SessionLifecycleState::Stopped && stopped.stopped_at.is_some(),
        "session stop state was not persisted"
    );
    let stopped_release_error = expected_store_error(
        store
            .release_session_runtime_for_owner(&owner, created.id)
            .await,
        "stopped session release should conflict",
    )?;
    ensure!(
        matches!(stopped_release_error, SessionStoreError::Conflict(_)),
        "stopped session release returned the wrong error class"
    );
    let stopped_active = store
        .mark_session_active(created.id)
        .await
        .context("attempt active transition after stop")?
        .context("stopped contract session disappeared")?;
    ensure!(
        stopped_active.state == SessionLifecycleState::Stopped,
        "stopped session accepted an active transition"
    );

    let resumed = store
        .prepare_session_for_connect(created.id)
        .await
        .context("prepare stopped session for reconnect")?
        .context("stopped session disappeared before reconnect")?;
    ensure!(
        resumed.state == SessionLifecycleState::Ready && resumed.stopped_at.is_none(),
        "stopped session did not return to ready"
    );
    let released = store
        .release_session_runtime_for_owner(&owner, created.id)
        .await
        .context("release ready session runtime")?
        .context("ready session disappeared before release")?;
    ensure!(
        released.state == SessionLifecycleState::Released && released.runtime_released_at.is_some(),
        "session runtime release was not persisted"
    );
    let resumed = store
        .prepare_session_for_connect(created.id)
        .await
        .context("prepare released session for reconnect")?
        .context("released session disappeared before reconnect")?;
    ensure!(
        resumed.state == SessionLifecycleState::Ready,
        "released session did not return to ready"
    );
    ensure!(
        store
            .prepare_session_for_connect(Uuid::now_v7())
            .await
            .context("prepare missing session")?
            .is_none(),
        "missing session unexpectedly became connectable"
    );
    Ok(())
}

fn session_request(project_id: Uuid) -> CreateSessionRequest {
    CreateSessionRequest {
        project_id: Some(project_id),
        idle_timeout_sec: Some(300),
        labels: HashMap::from([("contract".to_string(), "session".to_string())]),
        recording: SessionRecordingPolicy {
            mode: SessionRecordingMode::Manual,
            format: SessionRecordingFormat::Webm,
            retention_sec: Some(3_600),
        },
        ..CreateSessionRequest::default()
    }
}
