CREATE TABLE IF NOT EXISTS control_egress_profiles (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    proxy JSONB NULL,
    bypass_rules JSONB NOT NULL DEFAULT '[]'::jsonb,
    custom_ca JSONB NULL,
    state TEXT NOT NULL DEFAULT 'ready',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (owner_subject, owner_issuer, name)
);

CREATE INDEX IF NOT EXISTS control_egress_profiles_owner_idx
    ON control_egress_profiles (owner_subject, owner_issuer, created_at DESC);

ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS network_identity JSONB NOT NULL DEFAULT '{}'::jsonb;
