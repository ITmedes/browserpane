ALTER TABLE control_credential_bindings
    ADD COLUMN IF NOT EXISTS project_id UUID NULL REFERENCES control_projects(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_control_credential_bindings_project
    ON control_credential_bindings (owner_subject, owner_issuer, project_id, created_at DESC);
