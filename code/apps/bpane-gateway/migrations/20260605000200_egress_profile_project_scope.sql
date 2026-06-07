ALTER TABLE control_egress_profiles
    ADD COLUMN IF NOT EXISTS project_id UUID NULL REFERENCES control_projects(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_control_egress_profiles_owner_project_created
    ON control_egress_profiles (owner_subject, owner_issuer, project_id, created_at DESC);
