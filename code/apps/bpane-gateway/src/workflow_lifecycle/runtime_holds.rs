use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeReleaseOutcome {
    Released,
    Retry,
    AlreadyReleased,
    RunNoLongerAwaitingInput,
}

impl WorkflowLifecycleInner {
    pub(super) async fn reconcile_runtime_holds(
        self: &Arc<Self>,
    ) -> Result<(), WorkflowLifecycleError> {
        let runs = self
            .session_store
            .list_awaiting_input_workflow_runs()
            .await?;
        for run in runs {
            self.reconcile_runtime_hold(run.id).await?;
        }
        Ok(())
    }

    pub(super) async fn reconcile_runtime_hold(
        self: &Arc<Self>,
        run_id: Uuid,
    ) -> Result<(), WorkflowLifecycleError> {
        self.clear_runtime_hold_task(run_id).await;

        let Some(run) = self.session_store.get_workflow_run_by_id(run_id).await? else {
            return Ok(());
        };
        if run.state != WorkflowRunState::AwaitingInput {
            return Ok(());
        }

        let events = self.session_store.list_workflow_run_events(run_id).await?;
        let Some(awaiting_input_event) = latest_awaiting_input_event(&events) else {
            return Ok(());
        };
        if latest_runtime_release_event(&events, awaiting_input_event.created_at).is_some() {
            return Ok(());
        }

        let hold_request = awaiting_input_event
            .data
            .as_ref()
            .and_then(|value| parse_workflow_run_runtime_hold_request(value).ok())
            .flatten();

        if let Some(hold_request) = hold_request.as_ref() {
            self.ensure_runtime_held_event(run_id, hold_request, awaiting_input_event.created_at)
                .await?;
        }

        self.schedule_runtime_release(
            run.id,
            run.session_id,
            awaiting_input_event.created_at,
            hold_request,
        )
        .await;
        Ok(())
    }

    async fn ensure_runtime_held_event(
        &self,
        run_id: Uuid,
        hold_request: &WorkflowRunRuntimeHoldRequest,
        requested_at: chrono::DateTime<Utc>,
    ) -> Result<(), WorkflowLifecycleError> {
        let hold_until = requested_at
            + chrono::Duration::from_std(Duration::from_secs(hold_request.timeout_sec)).map_err(
                |error| {
                    WorkflowLifecycleError::InvalidConfiguration(format!(
                        "invalid workflow runtime hold timeout for run {run_id}: {error}"
                    ))
                },
            )?;
        let events = self.session_store.list_workflow_run_events(run_id).await?;
        let already_present = events.iter().rev().any(|event| {
            event.created_at >= requested_at && event.event_type == "workflow_run.runtime_held"
        });
        if already_present {
            return Ok(());
        }
        let _ = self
            .session_store
            .append_workflow_run_event(
                run_id,
                crate::workflow::PersistWorkflowRunEventRequest {
                    event_type: "workflow_run.runtime_held".to_string(),
                    message: "workflow run is holding the exact live runtime while awaiting input"
                        .to_string(),
                    data: Some(serde_json::json!({
                        "runtime_hold": {
                            "mode": "live",
                            "timeout_sec": hold_request.timeout_sec,
                            "hold_until": hold_until,
                        }
                    })),
                },
            )
            .await?;
        Ok(())
    }

    async fn clear_runtime_hold_task(&self, run_id: Uuid) {
        let handle = self.runtime_hold_tasks.lock().await.remove(&run_id);
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    async fn schedule_runtime_release(
        self: &Arc<Self>,
        run_id: Uuid,
        session_id: Uuid,
        requested_at: chrono::DateTime<Utc>,
        hold_request: Option<WorkflowRunRuntimeHoldRequest>,
    ) {
        let manager = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let release_reason = if let Some(hold_request) = hold_request {
                let hold_duration = Duration::from_secs(hold_request.timeout_sec);
                let requested_at_unix = requested_at.timestamp();
                let requested_at_nanos = requested_at.timestamp_subsec_nanos();
                let requested_at_system = if requested_at_unix >= 0 {
                    std::time::UNIX_EPOCH
                        + Duration::from_secs(requested_at_unix as u64)
                        + Duration::from_nanos(u64::from(requested_at_nanos))
                } else {
                    std::time::SystemTime::now()
                };
                let hold_until = requested_at_system + hold_duration;
                let remaining = hold_until
                    .duration_since(std::time::SystemTime::now())
                    .unwrap_or_else(|_| Duration::from_secs(0));
                tokio::time::sleep(remaining).await;
                "hold_expired"
            } else {
                "awaiting_input_no_live_hold"
            };

            loop {
                match manager
                    .try_release_runtime(run_id, session_id, requested_at, release_reason)
                    .await
                {
                    Ok(RuntimeReleaseOutcome::Released)
                    | Ok(RuntimeReleaseOutcome::RunNoLongerAwaitingInput)
                    | Ok(RuntimeReleaseOutcome::AlreadyReleased) => break,
                    Ok(RuntimeReleaseOutcome::Retry) => {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                    Err(error) => {
                        warn!(run_id = %run_id, session_id = %session_id, "failed to release workflow runtime hold: {error}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            let _ = manager.runtime_hold_tasks.lock().await.remove(&run_id);
        });
        self.runtime_hold_tasks.lock().await.insert(run_id, handle);
    }

    async fn try_release_runtime(
        &self,
        run_id: Uuid,
        session_id: Uuid,
        requested_at: chrono::DateTime<Utc>,
        release_reason: &str,
    ) -> Result<RuntimeReleaseOutcome, WorkflowLifecycleError> {
        let Some(run) = self.session_store.get_workflow_run_by_id(run_id).await? else {
            return Ok(RuntimeReleaseOutcome::RunNoLongerAwaitingInput);
        };
        if run.state != WorkflowRunState::AwaitingInput {
            return Ok(RuntimeReleaseOutcome::RunNoLongerAwaitingInput);
        }

        let events = self.session_store.list_workflow_run_events(run_id).await?;
        if latest_runtime_release_event(&events, requested_at).is_some() {
            return Ok(RuntimeReleaseOutcome::AlreadyReleased);
        }

        if let Some(snapshot) = self.registry.telemetry_snapshot_if_live(session_id).await {
            if snapshot.browser_clients > 0 || snapshot.viewer_clients > 0 || snapshot.mcp_owner {
                return Ok(RuntimeReleaseOutcome::Retry);
            }
        }

        let Some(session) = self.session_store.get_session_by_id(session_id).await? else {
            return Ok(RuntimeReleaseOutcome::AlreadyReleased);
        };
        let session_state_before = session.state;

        if session_state_before != SessionLifecycleState::Stopped {
            let _ = self.session_store.mark_session_idle(session_id).await;
            self.session_manager.mark_session_idle(session_id).await;
        }

        let stopped = self
            .session_store
            .stop_session_if_idle(session_id)
            .await?
            .map(|session| session.state == SessionLifecycleState::Stopped)
            .unwrap_or(false);

        if !stopped {
            return Ok(RuntimeReleaseOutcome::Retry);
        }

        self.session_manager.release(session_id).await;
        self.registry.remove_session(session_id).await;
        let _ = self
            .session_store
            .append_workflow_run_event(
                run_id,
                crate::workflow::PersistWorkflowRunEventRequest {
                    event_type: "workflow_run.runtime_released".to_string(),
                    message: "workflow run released the exact live runtime while awaiting input"
                        .to_string(),
                    data: Some(serde_json::json!({
                        "runtime_release": {
                            "reason": release_reason,
                            "released_session_state": session_state_before.as_str(),
                        }
                    })),
                },
            )
            .await?;
        Ok(RuntimeReleaseOutcome::Released)
    }
}

fn latest_awaiting_input_event(
    events: &[crate::workflow::StoredWorkflowRunEvent],
) -> Option<&crate::workflow::StoredWorkflowRunEvent> {
    events.iter().rev().find(|event| {
        event.event_type == "workflow_run.awaiting_input"
            || event.event_type == "automation_task.awaiting_input"
    })
}

fn latest_runtime_release_event(
    events: &[crate::workflow::StoredWorkflowRunEvent],
    requested_at: chrono::DateTime<Utc>,
) -> Option<&crate::workflow::StoredWorkflowRunEvent> {
    events.iter().rev().find(|event| {
        event.created_at >= requested_at && event.event_type == "workflow_run.runtime_released"
    })
}
