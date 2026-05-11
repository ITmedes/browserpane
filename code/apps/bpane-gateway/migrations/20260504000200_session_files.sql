CREATE TABLE IF NOT EXISTS control_session_files (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES control_sessions(id) ON DELETE CASCADE,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    media_type TEXT NULL,
    byte_count BIGINT NOT NULL CHECK (byte_count >= 0),
    sha256_hex TEXT NOT NULL,
    artifact_ref TEXT NOT NULL,
    source TEXT NOT NULL,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_control_session_files_session_created
    ON control_session_files (session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_control_session_files_owner_created
    ON control_session_files (owner_subject, owner_issuer, created_at DESC);
