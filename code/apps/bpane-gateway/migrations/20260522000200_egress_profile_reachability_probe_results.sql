CREATE TABLE IF NOT EXISTS control_egress_profile_reachability_probe_results (
    profile_id UUID PRIMARY KEY REFERENCES control_egress_profiles(id) ON DELETE CASCADE,
    reachability_collected BOOLEAN NOT NULL DEFAULT FALSE,
    reachability_healthy BOOLEAN NOT NULL DEFAULT FALSE,
    last_failure_reason TEXT NULL,
    observed_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS control_egress_profile_reachability_probe_results_observed_at_idx
    ON control_egress_profile_reachability_probe_results (observed_at DESC);
