ALTER TABLE control_workflow_runs
    ADD COLUMN IF NOT EXISTS project_id UUID NULL REFERENCES control_projects(id);

UPDATE control_workflow_runs run
SET project_id = session.project_id
FROM control_sessions session
WHERE run.session_id = session.id
  AND run.project_id IS NULL
  AND session.project_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS control_workflow_runs_project_state_idx
    ON control_workflow_runs (owner_subject, owner_issuer, project_id, state, created_at DESC);
