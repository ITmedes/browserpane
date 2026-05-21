CREATE TABLE IF NOT EXISTS control_browser_contexts (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    persistence_mode TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'ready',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS control_browser_contexts_owner_idx
    ON control_browser_contexts (owner_subject, owner_issuer, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS control_browser_contexts_owner_name_idx
    ON control_browser_contexts (owner_subject, owner_issuer, name);

ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS browser_context_mode TEXT NOT NULL DEFAULT 'fresh',
    ADD COLUMN IF NOT EXISTS browser_context_id UUID REFERENCES control_browser_contexts(id);

CREATE INDEX IF NOT EXISTS control_sessions_browser_context_idx
    ON control_sessions (browser_context_id);
