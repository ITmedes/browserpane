ALTER TABLE control_workflow_definition_versions
    ADD COLUMN IF NOT EXISTS package JSONB NULL;
