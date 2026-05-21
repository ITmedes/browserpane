CREATE TABLE IF NOT EXISTS control_session_templates (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    defaults JSONB NOT NULL DEFAULT '{}'::jsonb,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS control_session_templates_owner_idx
    ON control_session_templates (owner_subject, owner_issuer, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS control_session_templates_owner_name_idx
    ON control_session_templates (owner_subject, owner_issuer, name);
