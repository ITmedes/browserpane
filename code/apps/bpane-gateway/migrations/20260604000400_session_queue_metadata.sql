ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS queued_at TIMESTAMPTZ NULL;

UPDATE control_sessions
SET queued_at = COALESCE(queued_at, created_at)
WHERE state = 'queued'
  AND queued_at IS NULL;
