ALTER TABLE control_sessions
    ADD COLUMN IF NOT EXISTS capabilities JSONB NOT NULL DEFAULT '{"browser_input":true,"clipboard":true,"audio":true,"microphone":true,"camera":true,"file_transfer":true,"resize":true}'::jsonb;
