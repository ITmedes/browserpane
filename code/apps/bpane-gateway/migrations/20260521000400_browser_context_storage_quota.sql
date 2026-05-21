ALTER TABLE control_browser_contexts
    ADD COLUMN IF NOT EXISTS max_profile_storage_bytes BIGINT;
