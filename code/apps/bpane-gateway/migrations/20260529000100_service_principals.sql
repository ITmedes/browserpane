CREATE TABLE IF NOT EXISTS control_service_principals (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    client_id TEXT NOT NULL,
    issuer TEXT NOT NULL,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    scopes JSONB NOT NULL DEFAULT '[]'::jsonb,
    allowed_project_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    state TEXT NOT NULL DEFAULT 'active',
    last_seen_at TIMESTAMPTZ,
    last_delegated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (owner_subject, owner_issuer, issuer, client_id)
);

CREATE INDEX IF NOT EXISTS control_service_principals_owner_idx
    ON control_service_principals (owner_subject, owner_issuer, created_at DESC);

CREATE INDEX IF NOT EXISTS control_service_principals_external_identity_idx
    ON control_service_principals (owner_subject, owner_issuer, issuer, client_id);
