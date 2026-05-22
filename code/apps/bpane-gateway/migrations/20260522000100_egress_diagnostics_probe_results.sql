CREATE TABLE IF NOT EXISTS control_session_egress_diagnostics_probe_results (
    session_id UUID PRIMARY KEY REFERENCES control_sessions(id) ON DELETE CASCADE,
    profile_id UUID NULL,
    active_probe_collected BOOLEAN NOT NULL DEFAULT FALSE,
    observed_public_ip TEXT NULL,
    observed_tls_issuer TEXT NULL,
    last_failure_reason TEXT NULL,
    observed_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS control_session_egress_diagnostics_probe_results_profile_idx
    ON control_session_egress_diagnostics_probe_results (profile_id, observed_at DESC);
