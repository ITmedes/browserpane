CREATE TABLE IF NOT EXISTS control_session_file_bindings (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES control_file_workspaces(id) ON DELETE RESTRICT,
    file_id UUID NOT NULL REFERENCES control_file_workspace_files(id) ON DELETE RESTRICT,
    file_name TEXT NOT NULL,
    media_type TEXT NULL,
    byte_count BIGINT NOT NULL CHECK (byte_count >= 0),
    sha256_hex TEXT NOT NULL,
    provenance JSONB NULL,
    artifact_ref TEXT NOT NULL,
    mount_path TEXT NOT NULL,
    mode TEXT NOT NULL,
    state TEXT NOT NULL,
    error TEXT NULL,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_control_session_file_bindings_session_created
    ON control_session_file_bindings (session_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_control_session_file_bindings_active_mount_path
    ON control_session_file_bindings (session_id, mount_path)
    WHERE state <> 'removed';
