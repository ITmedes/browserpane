export type SessionRuntimeInfo = {
  readonly binding: string;
  readonly compatibility_mode: string;
  readonly cdp_endpoint?: string | null;
};

export type SessionViewport = {
  readonly width: number;
  readonly height: number;
};

export type SessionGeolocation = {
  readonly latitude: number;
  readonly longitude: number;
  readonly accuracy_meters?: number | null;
};

export type SessionNetworkIdentity = {
  readonly locale?: string | null;
  readonly languages?: readonly string[];
  readonly timezone?: string | null;
  readonly geolocation?: SessionGeolocation | null;
  readonly user_agent?: string | null;
  readonly browser_identity?: string | null;
  readonly egress_profile_id?: string | null;
};

export type EgressProfileState = 'ready' | 'disabled';

export type EgressTrafficObservationMode = 'metadata_only' | 'tls_intercept';

export type EgressProxyConfig = {
  readonly url: string;
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
  readonly bypass_rule_count: number;
  readonly custom_ca_configured: boolean;
  readonly tls_interception_enabled: boolean;
  readonly sensitive_log_sink_configured: boolean;
  readonly proof: EgressDiagnosticsProof;
  readonly warnings: readonly string[];
  readonly observed_at: string;
};

export type RunEgressDiagnosticsProbeCommand = {
  readonly public_ip_url?: string | null;
  readonly tls_probe_url?: string | null;
  readonly timeout_ms?: number | null;
};

export type SessionEffectiveEgress = {
  readonly profile_id?: string | null;
  readonly profile_name?: string | null;
  readonly profile_state?: EgressProfileState | null;
  readonly proxy_configured: boolean;
  readonly bypass_rule_count: number;
  readonly custom_ca_configured: boolean;
  readonly observation_mode: EgressTrafficObservationMode;
  readonly tls_interception_enabled: boolean;
  readonly sensitive_log_sink_configured: boolean;
};

export type EgressProfileResource = {
  readonly id: string;
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

export type CreateEgressProfileCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly proxy?: EgressProxyConfig | null;
  readonly bypass_rules?: readonly string[];
  readonly custom_ca?: EgressCustomCaConfig | null;
  readonly traffic_observation?: EgressTrafficObservationConfig;
  readonly state?: EgressProfileState;
};

export type BrowserContextState = 'ready' | 'deleted';

export type BrowserContextPersistenceMode = 'reusable' | 'ephemeral';

export type SessionBrowserContextMode = 'fresh' | 'ephemeral' | 'reusable';

export type SessionBrowserContextResource = {
  readonly mode: SessionBrowserContextMode;
  readonly context_id?: string | null;
};

export type SessionBrowserContextCommand = {
  readonly mode: SessionBrowserContextMode;
  readonly context_id?: string | null;
};

export type BrowserContextResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly persistence_mode: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly retention_expires_at?: string | null;
  readonly max_profile_storage_bytes?: number | null;
  readonly state: BrowserContextState;
  readonly usage?: BrowserContextUsageResource | null;
  readonly created_at: string;
  readonly updated_at: string;
  readonly last_used_at?: string | null;
  readonly deleted_at?: string | null;
};

export type BrowserContextUsageResource = {
  readonly visible_session_count: number;
  readonly active_runtime_session_count: number;
  readonly active_runtime_session_id?: string | null;
  readonly profile_storage_bytes?: number | null;
  readonly profile_storage_limit_exceeded: boolean;
};

export type BrowserContextListResponse = {
  readonly contexts: readonly BrowserContextResource[];
};

export type CreateBrowserContextCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly persistence_mode?: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type CloneBrowserContextCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type ImportBrowserContextCommand = {
  readonly name: string;
  readonly archive: BodyInit;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type SessionConnectInfo = {
  readonly gateway_url: string;
  readonly transport_path: string;
  readonly auth_type: string;
  readonly ticket_path?: string | null;
  readonly compatibility_mode: string;
};

export type SessionStopBlocker = {
  readonly kind: string;
  readonly count: number;
};

export type SessionStopEligibility = {
  readonly allowed: boolean;
  readonly blockers: readonly SessionStopBlocker[];
};

export type SessionAutomationDelegate = {
  readonly client_id: string;
  readonly issuer: string;
  readonly display_name?: string | null;
};

export type SessionConnectionCounts = {
  readonly interactive_clients: number;
  readonly owner_clients: number;
  readonly viewer_clients: number;
  readonly recorder_clients: number;
  readonly automation_clients: number;
  readonly total_clients: number;
};

export type SessionStatusSummary = {
  readonly runtime_state: string;
  readonly runtime_resume_mode: string;
  readonly presence_state: string;
  readonly connection_counts: SessionConnectionCounts;
  readonly stop_eligibility: SessionStopEligibility;
};

export type SessionResource = {
  readonly id: string;
  readonly state: string;
  readonly template_id?: string | null;
  readonly browser_context: SessionBrowserContextResource;
  readonly network_identity?: SessionNetworkIdentity;
  readonly effective_egress?: SessionEffectiveEgress;
  readonly egress_diagnostics?: EgressDiagnosticsResource;
  readonly owner_mode: string;
  readonly viewport?: SessionViewport | null;
  readonly idle_timeout_sec?: number | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
  readonly automation_delegate?: SessionAutomationDelegate | null;
  readonly connect: SessionConnectInfo;
  readonly runtime: SessionRuntimeInfo;
  readonly status: SessionStatusSummary;
  readonly created_at: string;
  readonly updated_at: string;
  readonly runtime_released_at?: string | null;
  readonly stopped_at?: string | null;
};

export type SessionListResponse = {
  readonly sessions: readonly SessionResource[];
};

export type SessionListFilters = {
  readonly templateId?: string | null;
  readonly states?: readonly string[];
  readonly runtimeStates?: readonly string[];
  readonly labels?: Readonly<Record<string, string>>;
  readonly integrationContext?: Readonly<Record<string, string>>;
  readonly limit?: number | null;
  readonly offset?: number | null;
};

export type SessionTemplateDefaults = {
  readonly owner_mode?: string | null;
  readonly viewport?: SessionViewport | null;
  readonly idle_timeout_sec?: number | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
  readonly network_identity?: SessionNetworkIdentity | null;
  readonly recording?: Readonly<Record<string, unknown>> | null;
};

export type SessionTemplateResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly defaults: SessionTemplateDefaults;
  readonly version: number;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionTemplateListResponse = {
  readonly templates: readonly SessionTemplateResource[];
};

export type CreateSessionCommand = {
  readonly template_id?: string | null;
  readonly browser_context?: SessionBrowserContextCommand;
  readonly network_identity?: SessionNetworkIdentity;
  readonly owner_mode?: string;
  readonly idle_timeout_sec?: number;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
};

export type SetAutomationDelegateCommand = {
  readonly client_id: string;
  readonly issuer?: string | null;
  readonly display_name?: string | null;
};

export type SessionAccessTokenResponse = {
  readonly session_id: string;
  readonly token_type: string;
  readonly token: string;
  readonly expires_at: string;
  readonly connect: SessionConnectInfo;
};

export type SessionFileResource = {
  readonly id: string;
  readonly session_id: string;
  readonly name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly source: string;
  readonly labels: Readonly<Record<string, string>>;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionFileListResponse = {
  readonly files: readonly SessionFileResource[];
};

export type FileWorkspaceResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly files_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type FileWorkspaceListResponse = {
  readonly workspaces: readonly FileWorkspaceResource[];
};

export type CreateFileWorkspaceCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
};

export type FileWorkspaceFileResource = {
  readonly id: string;
  readonly workspace_id: string;
  readonly name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly provenance: Readonly<Record<string, unknown>> | null;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type FileWorkspaceFileListResponse = {
  readonly files: readonly FileWorkspaceFileResource[];
};

export type UploadFileWorkspaceFileCommand = {
  readonly fileName: string;
  readonly content: BodyInit;
  readonly mediaType?: string | null;
  readonly provenance?: Readonly<Record<string, unknown>> | null;
};

export type SessionFileBindingMode = 'read_only' | 'read_write' | 'scratch_output';

export type SessionFileBindingState = 'pending' | 'materialized' | 'failed' | 'removed';

export type SessionFileBindingResource = {
  readonly id: string;
  readonly session_id: string;
  readonly workspace_id: string;
  readonly file_id: string;
  readonly file_name: string;
  readonly media_type?: string | null;
  readonly byte_count: number;
  readonly sha256_hex: string;
  readonly provenance: Readonly<Record<string, unknown>> | null;
  readonly mount_path: string;
  readonly mode: SessionFileBindingMode;
  readonly state: SessionFileBindingState;
  readonly error?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly content_path: string;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionFileBindingListResponse = {
  readonly bindings: readonly SessionFileBindingResource[];
};

export type CreateSessionFileBindingCommand = {
  readonly workspace_id: string;
  readonly file_id: string;
  readonly mount_path: string;
  readonly mode?: SessionFileBindingMode;
  readonly labels?: Readonly<Record<string, string>>;
};
