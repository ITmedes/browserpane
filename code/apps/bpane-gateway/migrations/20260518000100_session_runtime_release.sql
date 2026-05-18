ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS runtime_released_at TIMESTAMPTZ NULL;
