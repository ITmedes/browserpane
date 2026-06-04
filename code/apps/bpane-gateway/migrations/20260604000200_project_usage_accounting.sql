ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS runtime_started_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS runtime_usage_ms BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS egress_rx_bytes BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS egress_tx_bytes BIGINT NOT NULL DEFAULT 0;

UPDATE control_sessions
SET runtime_started_at = COALESCE(runtime_started_at, created_at)
WHERE state IN ('pending', 'starting', 'ready', 'active', 'idle')
  AND runtime_started_at IS NULL;
