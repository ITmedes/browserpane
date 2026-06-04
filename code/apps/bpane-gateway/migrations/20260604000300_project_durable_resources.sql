ALTER TABLE control_browser_contexts
    ADD COLUMN IF NOT EXISTS project_id UUID NULL REFERENCES control_projects(id) ON DELETE RESTRICT;

CREATE INDEX IF NOT EXISTS control_browser_contexts_project_idx
    ON control_browser_contexts (project_id, state, created_at DESC);

ALTER TABLE control_file_workspaces
    ADD COLUMN IF NOT EXISTS project_id UUID NULL REFERENCES control_projects(id) ON DELETE RESTRICT;

CREATE INDEX IF NOT EXISTS idx_control_file_workspaces_project_created
    ON control_file_workspaces (project_id, created_at DESC);
