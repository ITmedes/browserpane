ALTER TABLE control_egress_profiles
    ADD COLUMN IF NOT EXISTS traffic_observation JSONB NOT NULL DEFAULT '{"mode":"metadata_only"}'::jsonb;
