use anyhow::{ensure, Context};

use super::*;

pub(super) async fn run_recording_contracts(store: &SessionStore) -> anyhow::Result<()> {
    let suffix = Uuid::now_v7().simple().to_string();
    let owner = principal(&format!("recording-owner-{suffix}"));
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest {
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(3_600),
                },
                ..CreateSessionRequest::default()
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .context("create recording contract session")?;
    let other_session = store
        .create_session(
            &owner,
            CreateSessionRequest::default(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .context("create recording isolation session")?;

    let recording = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .context("create contract recording")?;
    ensure!(
        recording.state == SessionRecordingState::Recording
            && recording.previous_recording_id.is_none(),
        "new recording state diverged"
    );
    ensure!(
        store
            .list_recordings_for_session(session.id)
            .await
            .context("list contract recordings")?
            .iter()
            .any(|item| item.id == recording.id),
        "recording was missing from its session catalog"
    );
    ensure!(
        store
            .list_recordings_for_session(other_session.id)
            .await
            .context("list other session recordings")?
            .iter()
            .all(|item| item.id != recording.id),
        "recording leaked into another session catalog"
    );

    let stopped = store
        .stop_recording_for_session(
            session.id,
            recording.id,
            SessionRecordingTerminationReason::ManualStop,
        )
        .await
        .context("stop contract recording")?
        .context("contract recording disappeared before stop")?;
    ensure!(
        stopped.state == SessionRecordingState::Finalizing
            && stopped.termination_reason == Some(SessionRecordingTerminationReason::ManualStop),
        "recording stop/finalizing state diverged"
    );
    ensure!(
        store
            .complete_recording_for_session(
                other_session.id,
                recording.id,
                completed_recording_request(&suffix),
            )
            .await
            .context("complete recording through another session")?
            .is_none(),
        "recording was completed through another session"
    );
    let completed = store
        .complete_recording_for_session(
            session.id,
            recording.id,
            completed_recording_request(&suffix),
        )
        .await
        .context("complete contract recording")?
        .context("contract recording disappeared before completion")?;
    ensure!(
        completed.state == SessionRecordingState::Ready
            && completed.bytes == Some(1_024)
            && completed.duration_ms == Some(2_000)
            && completed.completed_at.is_some(),
        "recording completion metadata diverged"
    );

    let failed = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, Some(recording.id))
        .await
        .context("create linked contract recording")?;
    let failed = store
        .fail_recording_for_session(
            session.id,
            failed.id,
            FailSessionRecordingRequest {
                error: "contract recorder exit".to_string(),
                termination_reason: Some(SessionRecordingTerminationReason::WorkerExit),
            },
        )
        .await
        .context("fail linked contract recording")?
        .context("linked contract recording disappeared before failure")?;
    ensure!(
        failed.state == SessionRecordingState::Failed
            && failed.previous_recording_id == Some(recording.id)
            && failed.termination_reason == Some(SessionRecordingTerminationReason::WorkerExit),
        "recording failure linkage diverged"
    );
    ensure!(
        store
            .get_latest_recording_for_session(session.id)
            .await
            .context("get latest contract recording")?
            .is_some_and(|latest| latest.id == failed.id),
        "latest recording selection diverged"
    );
    ensure!(
        store
            .stop_recording_for_session(
                session.id,
                Uuid::now_v7(),
                SessionRecordingTerminationReason::ManualStop,
            )
            .await
            .context("stop missing contract recording")?
            .is_none(),
        "missing recording unexpectedly transitioned"
    );
    Ok(())
}

fn completed_recording_request(suffix: &str) -> PersistCompletedSessionRecordingRequest {
    PersistCompletedSessionRecordingRequest {
        artifact_ref: format!("local_fs:contract/{suffix}/recording.webm"),
        mime_type: Some("video/webm".to_string()),
        bytes: Some(1_024),
        duration_ms: Some(2_000),
    }
}
