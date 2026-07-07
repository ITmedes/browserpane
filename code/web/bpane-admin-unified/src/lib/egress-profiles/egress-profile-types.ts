import type { ProjectState } from '$lib/projects/project-types';

export type EgressProfileState = 'ready' | 'disabled';

export type EgressTrafficObservationMode = 'metadata_only' | 'tls_intercept';

export type EgressProxyConfig = {
  readonly url: string;
  readonly credential_binding_id?: string | null;
};
export type EgressCustomCaConfig = {
  readonly certificate_ref: string;
  readonly display_name?: string | null;
};

export type EgressTrafficObservationConfig = {
  readonly mode: EgressTrafficObservationMode;
  readonly sensitive_log_sink_ref?: string | null;
  readonly sensitive_log_sink_display_name?: string | null;
};

export type EgressProfileEffectiveStatus = {
  readonly proxy_configured: boolean;
  readonly proxy_auth_configured: boolean;
  readonly bypass_rule_count: number;
  readonly custom_ca_configured: boolean;
  readonly observation_mode: EgressTrafficObservationMode;
  readonly tls_interception_enabled: boolean;
  readonly sensitive_log_sink_configured: boolean;
};

export type EgressDiagnosticsHealth = 'ready' | 'unknown' | 'attention' | 'blocked' | 'missing';

export type EgressDiagnosticsProofLevel = 'none' | 'configuration' | 'runtime_launch_metadata' | 'active_probe';

export type EgressDiagnosticsProof = {
  readonly profile_resolved: boolean;
  readonly profile_ready: boolean;
  readonly profile_reachability_collected: boolean;
  readonly profile_reachability_healthy: boolean;
  readonly profile_reachability_observed_at?: string | null;
  readonly profile_reachability_failure?: string | null;
  readonly proxy_launch_config_expected: boolean;
  readonly bypass_rules_expected: number;
  readonly custom_ca_launch_config_expected: boolean;
  readonly tls_interception_expected: boolean;
  readonly sensitive_log_sink_declared: boolean;
  readonly runtime_launch_observed: boolean;
  readonly active_probe_collected: boolean;
  readonly observed_public_ip?: string | null;
  readonly observed_tls_issuer?: string | null;
  readonly last_failure_reason?: string | null;
};

export type EgressDiagnosticsResource = {
  readonly profile_id?: string | null;
  readonly profile_name?: string | null;
  readonly profile_state?: EgressProfileState | null;
  readonly health: EgressDiagnosticsHealth;
  readonly observation_mode: EgressTrafficObservationMode;
  readonly proof_level: EgressDiagnosticsProofLevel;
  readonly runtime_binding?: string | null;
  readonly runtime_assignment?: string | null;
  readonly proxy_configured: boolean;
  readonly proxy_auth_configured: boolean;
  readonly bypass_rule_count: number;
  readonly custom_ca_configured: boolean;
  readonly tls_interception_enabled: boolean;
  readonly sensitive_log_sink_configured: boolean;
  readonly proof: EgressDiagnosticsProof;
  readonly warnings: readonly string[];
  readonly observed_at: string;
};

export type EgressProfileProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly state: ProjectState;
};

export type EgressProfileResource = {
  readonly id: string;
  readonly project_id?: string | null;
  readonly project?: EgressProfileProjectResource | null;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly proxy?: EgressProxyConfig | null;
  readonly bypass_rules: readonly string[];
  readonly custom_ca?: EgressCustomCaConfig | null;
  readonly traffic_observation: EgressTrafficObservationConfig;
  readonly state: EgressProfileState;
  readonly effective: EgressProfileEffectiveStatus;
  readonly diagnostics: EgressDiagnosticsResource;
  readonly created_at: string;
  readonly updated_at: string;
};

export type EgressProfileListResponse = {
  readonly profiles: readonly EgressProfileResource[];
};

export type UpsertEgressProfileRequest = {
  readonly project_id?: string | null;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly proxy?: EgressProxyConfig | null;
  readonly bypass_rules: readonly string[];
  readonly custom_ca?: EgressCustomCaConfig | null;
  readonly traffic_observation: EgressTrafficObservationConfig;
  readonly state: EgressProfileState;
};

export type EgressProfileProjectOptionsResponse = {
  readonly projects: readonly EgressProfileProjectResource[];
};
