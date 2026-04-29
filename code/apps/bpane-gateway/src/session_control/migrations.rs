use super::*;

pub(super) async fn run_postgres_migrations(
    client: &Mutex<Client>,
) -> Result<(), SessionStoreError> {
    client
        .lock()
        .await
        .batch_execute(
            r#"
            CREATE TABLE IF NOT EXISTS control_sessions (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                owner_display_name TEXT NULL,
                automation_owner_client_id TEXT NULL,
                automation_owner_issuer TEXT NULL,
                automation_owner_display_name TEXT NULL,
                state TEXT NOT NULL,
                template_id TEXT NULL,
                owner_mode TEXT NOT NULL,
                viewport_width INTEGER NOT NULL CHECK (viewport_width > 0 AND viewport_width <= 65535),
                viewport_height INTEGER NOT NULL CHECK (viewport_height > 0 AND viewport_height <= 65535),
                idle_timeout_sec INTEGER NULL CHECK (idle_timeout_sec IS NULL OR idle_timeout_sec > 0),
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                integration_context JSONB NULL,
                extensions JSONB NOT NULL DEFAULT '[]'::jsonb,
                recording JSONB NOT NULL DEFAULT '{"mode":"disabled","format":"webm","retention_sec":null}'::jsonb,
                runtime_binding TEXT NOT NULL DEFAULT 'legacy_single_session',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                stopped_at TIMESTAMPTZ NULL
            );

            CREATE INDEX IF NOT EXISTS idx_control_sessions_owner_created
                ON control_sessions (owner_subject, owner_issuer, created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_control_sessions_runtime_state
                ON control_sessions (runtime_binding, state, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_session_runtimes (
                session_id UUID PRIMARY KEY REFERENCES control_sessions(id) ON DELETE CASCADE,
                runtime_binding TEXT NOT NULL,
                status TEXT NOT NULL,
                agent_socket_path TEXT NOT NULL,
                container_name TEXT NULL,
                cdp_endpoint TEXT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_session_runtimes_binding_updated
                ON control_session_runtimes (runtime_binding, updated_at DESC);

            CREATE TABLE IF NOT EXISTS control_session_recordings (
                id UUID PRIMARY KEY,
                session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                previous_recording_id UUID NULL REFERENCES control_session_recordings(id) ON DELETE SET NULL,
                state TEXT NOT NULL,
                format TEXT NOT NULL,
                mime_type TEXT NULL,
                byte_count BIGINT NULL CHECK (byte_count IS NULL OR byte_count >= 0),
                duration_ms BIGINT NULL CHECK (duration_ms IS NULL OR duration_ms >= 0),
                error TEXT NULL,
                termination_reason TEXT NULL,
                artifact_path TEXT NULL,
                started_at TIMESTAMPTZ NOT NULL,
                completed_at TIMESTAMPTZ NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_session_recordings_session_created
                ON control_session_recordings (session_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_session_recording_workers (
                session_id UUID PRIMARY KEY REFERENCES control_sessions(id) ON DELETE CASCADE,
                recording_id UUID NOT NULL REFERENCES control_session_recordings(id) ON DELETE CASCADE,
                status TEXT NOT NULL,
                process_id BIGINT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_session_recording_workers_updated
                ON control_session_recording_workers (updated_at DESC);

            CREATE TABLE IF NOT EXISTS control_automation_tasks (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                owner_display_name TEXT NULL,
                display_name TEXT NULL,
                executor TEXT NOT NULL,
                state TEXT NOT NULL,
                session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                session_source TEXT NOT NULL,
                input JSONB NULL,
                output JSONB NULL,
                error TEXT NULL,
                artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                cancel_requested_at TIMESTAMPTZ NULL,
                started_at TIMESTAMPTZ NULL,
                completed_at TIMESTAMPTZ NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_automation_tasks_owner_created
                ON control_automation_tasks (owner_subject, owner_issuer, created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_control_automation_tasks_session_created
                ON control_automation_tasks (session_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_automation_task_events (
                id UUID PRIMARY KEY,
                task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                event_type TEXT NOT NULL,
                message TEXT NOT NULL,
                data JSONB NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_automation_task_events_task_created
                ON control_automation_task_events (task_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS control_automation_task_logs (
                id UUID PRIMARY KEY,
                task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                stream TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_automation_task_logs_task_created
                ON control_automation_task_logs (task_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS control_workflow_definitions (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                owner_display_name TEXT NULL,
                name TEXT NOT NULL,
                description TEXT NULL,
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                latest_version TEXT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_definitions_owner_created
                ON control_workflow_definitions (owner_subject, owner_issuer, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_workflow_definition_versions (
                id UUID PRIMARY KEY,
                workflow_definition_id UUID NOT NULL REFERENCES control_workflow_definitions(id) ON DELETE CASCADE,
                version TEXT NOT NULL,
                executor TEXT NOT NULL,
                entrypoint TEXT NOT NULL,
                source JSONB NULL,
                input_schema JSONB NULL,
                output_schema JSONB NULL,
                default_session JSONB NULL,
                allowed_credential_binding_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                allowed_extension_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                allowed_file_workspace_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (workflow_definition_id, version)
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_definition_versions_workflow_created
                ON control_workflow_definition_versions (workflow_definition_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_credential_bindings (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                external_ref TEXT NOT NULL,
                namespace TEXT NULL,
                allowed_origins JSONB NOT NULL DEFAULT '[]'::jsonb,
                injection_mode TEXT NOT NULL,
                totp JSONB NULL,
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_credential_bindings_owner_created
                ON control_credential_bindings (owner_subject, owner_issuer, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_extensions (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NULL,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                latest_version TEXT NULL,
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_extensions_owner_created
                ON control_extensions (owner_subject, owner_issuer, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_extension_versions (
                id UUID PRIMARY KEY,
                extension_definition_id UUID NOT NULL REFERENCES control_extensions(id) ON DELETE CASCADE,
                version TEXT NOT NULL,
                install_path TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (extension_definition_id, version)
            );

            CREATE INDEX IF NOT EXISTS idx_control_extension_versions_extension_created
                ON control_extension_versions (extension_definition_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_workflow_runs (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                workflow_definition_id UUID NOT NULL REFERENCES control_workflow_definitions(id) ON DELETE CASCADE,
                workflow_definition_version_id UUID NOT NULL REFERENCES control_workflow_definition_versions(id) ON DELETE RESTRICT,
                workflow_version TEXT NOT NULL,
                session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                automation_task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                state TEXT NOT NULL DEFAULT 'pending',
                source_system TEXT NULL,
                source_reference TEXT NULL,
                client_request_id TEXT NULL,
                create_request_fingerprint TEXT NULL,
                source_snapshot JSONB NULL,
                extensions JSONB NOT NULL DEFAULT '[]'::jsonb,
                credential_bindings JSONB NOT NULL DEFAULT '[]'::jsonb,
                workspace_inputs JSONB NOT NULL DEFAULT '[]'::jsonb,
                produced_files JSONB NOT NULL DEFAULT '[]'::jsonb,
                input JSONB NULL,
                output JSONB NULL,
                error TEXT NULL,
                artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                started_at TIMESTAMPTZ NULL,
                completed_at TIMESTAMPTZ NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_runs_definition_created
                ON control_workflow_runs (workflow_definition_id, created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_control_workflow_runs_task
                ON control_workflow_runs (automation_task_id);

            CREATE TABLE IF NOT EXISTS control_workflow_run_workers (
                run_id UUID PRIMARY KEY REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
                automation_task_id UUID NOT NULL REFERENCES control_automation_tasks(id) ON DELETE CASCADE,
                status TEXT NOT NULL,
                process_id BIGINT NULL,
                container_name TEXT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_run_workers_updated
                ON control_workflow_run_workers (updated_at DESC);

            CREATE TABLE IF NOT EXISTS control_workflow_run_events (
                id UUID PRIMARY KEY,
                run_id UUID NOT NULL REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                event_type TEXT NOT NULL,
                message TEXT NOT NULL,
                data JSONB NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_run_events_run_created
                ON control_workflow_run_events (run_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS control_workflow_run_logs (
                id UUID PRIMARY KEY,
                run_id UUID NOT NULL REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                stream TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_run_logs_run_created
                ON control_workflow_run_logs (run_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS control_workflow_event_subscriptions (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                name TEXT NOT NULL,
                target_url TEXT NOT NULL,
                event_types JSONB NOT NULL DEFAULT '[]'::jsonb,
                signing_secret TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_event_subscriptions_owner_created
                ON control_workflow_event_subscriptions (owner_subject, owner_issuer, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_workflow_event_deliveries (
                id UUID PRIMARY KEY,
                subscription_id UUID NOT NULL REFERENCES control_workflow_event_subscriptions(id) ON DELETE CASCADE,
                run_id UUID NOT NULL REFERENCES control_workflow_runs(id) ON DELETE CASCADE,
                event_id UUID NOT NULL REFERENCES control_workflow_run_events(id) ON DELETE CASCADE,
                event_type TEXT NOT NULL,
                target_url TEXT NOT NULL,
                signing_secret TEXT NOT NULL,
                payload JSONB NOT NULL,
                state TEXT NOT NULL,
                attempt_count INTEGER NOT NULL DEFAULT 0,
                next_attempt_at TIMESTAMPTZ NULL,
                last_attempt_at TIMESTAMPTZ NULL,
                delivered_at TIMESTAMPTZ NULL,
                last_response_status INTEGER NULL,
                last_error TEXT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_event_deliveries_subscription_created
                ON control_workflow_event_deliveries (subscription_id, created_at ASC);

            CREATE INDEX IF NOT EXISTS idx_control_workflow_event_deliveries_due
                ON control_workflow_event_deliveries (state, next_attempt_at ASC, created_at ASC);

            CREATE TABLE IF NOT EXISTS control_workflow_event_delivery_attempts (
                id UUID PRIMARY KEY,
                delivery_id UUID NOT NULL REFERENCES control_workflow_event_deliveries(id) ON DELETE CASCADE,
                attempt_number INTEGER NOT NULL,
                response_status INTEGER NULL,
                error TEXT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_workflow_event_delivery_attempts_delivery_created
                ON control_workflow_event_delivery_attempts (delivery_id, created_at ASC);

            CREATE TABLE IF NOT EXISTS control_file_workspaces (
                id UUID PRIMARY KEY,
                owner_subject TEXT NOT NULL,
                owner_issuer TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NULL,
                labels JSONB NOT NULL DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_file_workspaces_owner_created
                ON control_file_workspaces (owner_subject, owner_issuer, created_at DESC);

            CREATE TABLE IF NOT EXISTS control_file_workspace_files (
                id UUID PRIMARY KEY,
                workspace_id UUID NOT NULL REFERENCES control_file_workspaces(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                media_type TEXT NULL,
                byte_count BIGINT NOT NULL CHECK (byte_count >= 0),
                sha256_hex TEXT NOT NULL,
                provenance JSONB NULL,
                artifact_ref TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_control_file_workspace_files_workspace_created
                ON control_file_workspace_files (workspace_id, created_at DESC);

            ALTER TABLE control_sessions
                ADD COLUMN IF NOT EXISTS automation_owner_client_id TEXT NULL;
            ALTER TABLE control_sessions
                ADD COLUMN IF NOT EXISTS automation_owner_issuer TEXT NULL;
            ALTER TABLE control_sessions
                ADD COLUMN IF NOT EXISTS automation_owner_display_name TEXT NULL;
            ALTER TABLE control_sessions
                ADD COLUMN IF NOT EXISTS extensions JSONB NOT NULL DEFAULT '[]'::jsonb;
            ALTER TABLE control_sessions
                ADD COLUMN IF NOT EXISTS recording JSONB NOT NULL DEFAULT '{"mode":"disabled","format":"webm","retention_sec":null}'::jsonb;
            ALTER TABLE control_session_recordings
                ADD COLUMN IF NOT EXISTS previous_recording_id UUID NULL REFERENCES control_session_recordings(id) ON DELETE SET NULL;
            ALTER TABLE control_session_recordings
                ADD COLUMN IF NOT EXISTS termination_reason TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS owner_subject TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS owner_issuer TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS state TEXT NOT NULL DEFAULT 'pending';
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS source_system TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS source_reference TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS client_request_id TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS create_request_fingerprint TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS source_snapshot JSONB NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS extensions JSONB NOT NULL DEFAULT '[]'::jsonb;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS credential_bindings JSONB NOT NULL DEFAULT '[]'::jsonb;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS workspace_inputs JSONB NOT NULL DEFAULT '[]'::jsonb;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS produced_files JSONB NOT NULL DEFAULT '[]'::jsonb;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS output JSONB NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS error TEXT NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS artifact_refs JSONB NOT NULL DEFAULT '[]'::jsonb;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ NULL;
            ALTER TABLE control_workflow_runs
                ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ NULL;
            UPDATE control_workflow_runs run
            SET owner_subject = task.owner_subject,
                owner_issuer = task.owner_issuer
            FROM control_automation_tasks task
            WHERE task.id = run.automation_task_id
              AND (run.owner_subject IS NULL OR run.owner_issuer IS NULL);
            ALTER TABLE control_workflow_runs
                ALTER COLUMN owner_subject SET NOT NULL;
            ALTER TABLE control_workflow_runs
                ALTER COLUMN owner_issuer SET NOT NULL;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_control_workflow_runs_owner_client_request
                ON control_workflow_runs (owner_subject, owner_issuer, client_request_id)
                WHERE client_request_id IS NOT NULL;
            ALTER TABLE control_workflow_definition_versions
                ADD COLUMN IF NOT EXISTS source JSONB NULL;
            "#,
        )
        .await
        .map_err(|error| SessionStoreError::Backend(format!("failed to migrate postgres schema: {error}")))
}
