CREATE TABLE IF NOT EXISTS control_identity_mappings (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    mapping_kind TEXT NOT NULL,
    issuer TEXT NOT NULL,
    external_id TEXT NOT NULL,
    claim_name TEXT,
    service_principal_id UUID REFERENCES control_service_principals(id),
    project_id UUID NOT NULL REFERENCES control_projects(id),
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    scopes JSONB NOT NULL DEFAULT '[]'::jsonb,
    state TEXT NOT NULL DEFAULT 'active',
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS control_identity_mappings_unique_idx
    ON control_identity_mappings (
        owner_subject,
        owner_issuer,
        mapping_kind,
        issuer,
        external_id,
        COALESCE(claim_name, ''),
        project_id
    );

CREATE INDEX IF NOT EXISTS control_identity_mappings_owner_idx
    ON control_identity_mappings (owner_subject, owner_issuer, created_at DESC);

CREATE INDEX IF NOT EXISTS control_identity_mappings_project_idx
    ON control_identity_mappings (owner_subject, owner_issuer, project_id);

CREATE INDEX IF NOT EXISTS control_identity_mappings_service_principal_idx
    ON control_identity_mappings (owner_subject, owner_issuer, service_principal_id)
    WHERE service_principal_id IS NOT NULL;
