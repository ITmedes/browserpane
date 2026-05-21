ALTER TABLE control_browser_contexts
    ADD COLUMN IF NOT EXISTS retention_sec BIGINT;
