CREATE TABLE IF NOT EXISTS control_workflow_endpoints (
    id UUID PRIMARY KEY,
    owner_subject TEXT NOT NULL,
    owner_issuer TEXT NOT NULL,
    project_id UUID NOT NULL REFERENCES control_projects(id),
    endpoint_key TEXT NOT NULL,
    purpose TEXT NOT NULL,
    workflow_definition_id UUID NOT NULL REFERENCES control_workflow_definitions(id),
    workflow_definition_version_id UUID NOT NULL REFERENCES control_workflow_definition_versions(id),
    workflow_version TEXT NOT NULL,
    input_schema JSONB NOT NULL,
    output_schema JSONB NOT NULL,
    execution_timeout_seconds INTEGER NOT NULL,
    inline_result_max_bytes INTEGER NOT NULL,
    artifact_behavior JSONB NOT NULL,
    labels JSONB NOT NULL DEFAULT '{}'::jsonb,
    state TEXT NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (project_id, endpoint_key),
    CHECK (state IN ('draft', 'active', 'disabled')),
    CHECK (execution_timeout_seconds BETWEEN 1 AND 86400),
    CHECK (inline_result_max_bytes BETWEEN 1 AND 1048576)
);

CREATE INDEX IF NOT EXISTS control_workflow_endpoints_owner_project_idx
    ON control_workflow_endpoints (owner_subject, owner_issuer, project_id, endpoint_key);

CREATE TABLE IF NOT EXISTS control_workflow_endpoint_grants (
    id UUID PRIMARY KEY,
    endpoint_id UUID NOT NULL REFERENCES control_workflow_endpoints(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES control_projects(id),
    service_principal_id UUID NOT NULL REFERENCES control_service_principals(id),
    operations JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (endpoint_id, service_principal_id)
);

CREATE INDEX IF NOT EXISTS control_workflow_endpoint_grants_lookup_idx
    ON control_workflow_endpoint_grants (endpoint_id, service_principal_id);

ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_id UUID NULL REFERENCES control_workflow_endpoints(id);
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_invocation_id UUID NULL;
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_key TEXT NULL;
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS caller_service_principal_id UUID NULL REFERENCES control_service_principals(id);
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_idempotency_key TEXT NULL;
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_request_fingerprint TEXT NULL;
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS execution_deadline_at TIMESTAMPTZ NULL;
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_outcome JSONB NULL;
ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS endpoint_side_effect_state TEXT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS control_workflow_runs_endpoint_invocation_idx
    ON control_workflow_runs (endpoint_invocation_id)
    WHERE endpoint_invocation_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS control_workflow_endpoint_invocations (
    id UUID PRIMARY KEY,
    endpoint_id UUID NOT NULL REFERENCES control_workflow_endpoints(id),
    caller_service_principal_id UUID NOT NULL REFERENCES control_service_principals(id),
    idempotency_key TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    run_id UUID NULL REFERENCES control_workflow_runs(id),
    outcome JSONB NULL,
    side_effect_state TEXT NOT NULL DEFAULT 'none',
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (endpoint_id, caller_service_principal_id, idempotency_key),
    CHECK (side_effect_state IN ('none', 'confirmed', 'uncertain'))
);

CREATE INDEX IF NOT EXISTS control_workflow_endpoint_invocations_run_idx
    ON control_workflow_endpoint_invocations (run_id)
    WHERE run_id IS NOT NULL;
