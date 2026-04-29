use super::*;

mod automation_tasks;
mod credential_bindings;
mod db;
mod extensions;
mod file_workspaces;
mod recordings;
mod runtime_assignments;
mod sessions;
mod workflow_definitions;
mod workflow_events;
mod workflow_runs;

use db::*;

pub(super) struct PostgresSessionStore {
    pub(super) db: PostgresDb,
    config: SessionStoreConfig,
}

impl PostgresSessionStore {
    pub(super) async fn connect(
        database_url: &str,
        config: SessionStoreConfig,
    ) -> Result<Self, SessionStoreError> {
        let db = PostgresDb::connect(database_url).await?;
        Ok(Self { db, config })
    }

    pub(super) async fn enqueue_workflow_event_deliveries(
        transaction: &Transaction<'_>,
        run: &StoredWorkflowRun,
        event: &StoredWorkflowRunEvent,
    ) -> Result<(), SessionStoreError> {
        let rows = transaction
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                "#,
                &[&run.owner_subject, &run.owner_issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow event subscriptions for delivery enqueue: {error}"
                ))
            })?;

        for row in rows {
            let subscription = row_to_stored_workflow_event_subscription(&row)?;
            if !workflow_event_type_matches(&subscription.event_types, &event.event_type) {
                continue;
            }
            let delivery_id = Uuid::now_v7();
            let payload =
                build_workflow_event_delivery_payload(subscription.id, delivery_id, run, event);
            transaction
                .execute(
                    r#"
                    INSERT INTO control_workflow_event_deliveries (
                        id,
                        subscription_id,
                        run_id,
                        event_id,
                        event_type,
                        target_url,
                        signing_secret,
                        payload,
                        state,
                        attempt_count,
                        next_attempt_at,
                        last_attempt_at,
                        delivered_at,
                        last_response_status,
                        last_error,
                        created_at,
                        updated_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8::jsonb, 'pending',
                        0, $9, NULL, NULL, NULL, NULL, $9, $9
                    )
                    "#,
                    &[
                        &delivery_id,
                        &subscription.id,
                        &run.id,
                        &event.id,
                        &event.event_type,
                        &subscription.target_url,
                        &subscription.signing_secret,
                        &payload,
                        &event.created_at,
                    ],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow event delivery: {error}"
                    ))
                })?;
        }
        Ok(())
    }
}
