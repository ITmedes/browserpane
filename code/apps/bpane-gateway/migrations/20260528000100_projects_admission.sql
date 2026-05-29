CREATE TABLE IF NOT EXISTS control_projects (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    quotas JSONB NOT NULL DEFAULT '{}'::jsonb,
    state TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (owner_subject, owner_issuer, name)
);

CREATE INDEX IF NOT EXISTS control_projects_owner_idx
    ON control_projects (owner_subject, owner_issuer, created_at DESC);

ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS project_id UUID NULL REFERENCES control_projects(id);

ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS admission JSONB NOT NULL DEFAULT jsonb_build_object(
        'state', 'allowed',
        'reason_code', 'owner_scope_unbounded',
        'message', 'No project was selected; owner-scoped admission is unbounded.',
        'project_id', NULL,
        'active_sessions', NULL,
        'max_active_sessions', NULL,
        'checked_at', to_jsonb(NOW())
    );

CREATE INDEX IF NOT EXISTS control_sessions_project_idx
    ON control_sessions (project_id, state, created_at DESC);
