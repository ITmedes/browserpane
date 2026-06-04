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

export type RunEgressDiagnosticsProbeCommand = {
  readonly public_ip_url?: string | null;
  readonly tls_probe_url?: string | null;
  readonly timeout_ms?: number | null;
};

export type RunEgressProfileReachabilityProbeCommand = {
  readonly timeout_ms?: number | null;
};

export type SessionEffectiveEgress = {
  readonly profile_id?: string | null;
  readonly profile_name?: string | null;
  readonly profile_state?: EgressProfileState | null;
  readonly proxy_configured: boolean;
  readonly proxy_auth_configured: boolean;
  readonly bypass_rule_count: number;
  readonly custom_ca_configured: boolean;
  readonly observation_mode: EgressTrafficObservationMode;
  readonly tls_interception_enabled: boolean;
  readonly sensitive_log_sink_configured: boolean;
};

export type ProjectState = 'active' | 'archived';

export type ProjectUsageBudgetEnforcement = 'warning_only' | 'block_session_creation';

export type ProjectQuotas = {
  readonly max_active_sessions?: number | null;
  readonly max_active_workflow_runs?: number | null;
  readonly max_retained_storage_bytes?: number | null;
  readonly max_session_creations?: number | null;
  readonly max_runtime_usage_ms?: number | null;
  readonly max_egress_total_bytes?: number | null;
};

export type ProjectPolicy = {
  readonly allowed_session_template_ids: readonly string[];
  readonly allowed_egress_profile_ids: readonly string[];
  readonly usage_budget_enforcement: ProjectUsageBudgetEnforcement;
};

export type ProjectUsageResource = {
  readonly project_id: string;
  readonly active_sessions: number;
  readonly queued_sessions: number;
  readonly session_creations: number;
  readonly max_session_creations?: number | null;
  readonly max_active_sessions?: number | null;
  readonly active_workflow_runs: number;
  readonly max_active_workflow_runs?: number | null;
  readonly runtime_usage_ms: number;
  readonly max_runtime_usage_ms?: number | null;
  readonly egress_rx_bytes: number;
  readonly egress_tx_bytes: number;
  readonly egress_total_bytes: number;
  readonly max_egress_total_bytes?: number | null;
  readonly retained_storage_bytes: number;
  readonly max_retained_storage_bytes?: number | null;
  readonly alerts: readonly ProjectUsageAlertResource[];
  readonly observed_at: string;
};

export type ProjectUsageAlertMetric = 'session_creations' | 'runtime_usage_ms' | 'egress_total_bytes';

export type ProjectUsageAlertState = 'approaching_limit' | 'exceeded';

export type ProjectUsageAlertResource = {
  readonly metric: ProjectUsageAlertMetric;
  readonly state: ProjectUsageAlertState;
  readonly current_value: number;
  readonly limit_value: number;
  readonly threshold_percent: number;
  readonly message: string;
};

export type ProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly quotas: ProjectQuotas;
  readonly policy: ProjectPolicy;
  readonly state: ProjectState;
  readonly usage: ProjectUsageResource;
  readonly created_at: string;
  readonly updated_at: string;
};

export type SessionProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly state: ProjectState;
};

export type ProjectAdmissionState = 'allowed' | 'queued' | 'rejected';

export type ProjectAdmissionReasonCode =
  | 'owner_scope_unbounded'
  | 'project_quota_available'
  | 'active_session_quota_exceeded'
  | 'session_creation_budget_exceeded'
  | 'active_workflow_run_quota_exceeded'
  | 'project_archived'
  | 'session_template_not_allowed'
  | 'egress_profile_not_allowed';

export type ProjectAdmissionDecision = {
  readonly state: ProjectAdmissionState;
  readonly reason_code: ProjectAdmissionReasonCode;
  readonly message: string;
  readonly project_id?: string | null;
  readonly active_sessions?: number | null;
  readonly max_active_sessions?: number | null;
  readonly active_workflow_runs?: number | null;
  readonly max_active_workflow_runs?: number | null;
  readonly session_creations?: number | null;
  readonly max_session_creations?: number | null;
  readonly checked_at: string;
};

export type ProjectListResponse = {
  readonly projects: readonly ProjectResource[];
};

export type ServicePrincipalState = 'active' | 'disabled';

export type ServicePrincipalResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly client_id: string;
  readonly issuer: string;
  readonly labels: Readonly<Record<string, string>>;
  readonly scopes: readonly string[];
  readonly allowed_project_ids: readonly string[];
  readonly state: ServicePrincipalState;
  readonly last_seen_at?: string | null;
  readonly last_delegated_at?: string | null;
  readonly created_at: string;
  readonly updated_at: string;
};

export type ServicePrincipalListResponse = {
  readonly service_principals: readonly ServicePrincipalResource[];
};

export type IdentityMappingKind = 'user' | 'group' | 'claim' | 'service_principal';

export type IdentityMappingState = 'active' | 'disabled';

export type IdentityMappingResource = {
  readonly id: string;
  readonly name: string;
  readonly description?: string | null;
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly external_id: string;
  readonly claim_name?: string | null;
  readonly service_principal_id?: string | null;
  readonly project_id: string;
  readonly labels: Readonly<Record<string, string>>;
  readonly scopes: readonly string[];
  readonly state: IdentityMappingState;
  readonly last_seen_at?: string | null;
  readonly created_at: string;
  readonly updated_at: string;
};

export type IdentityMappingListResponse = {
  readonly identity_mappings: readonly IdentityMappingResource[];
};

export type CreateIdentityMappingCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly external_id: string;
  readonly claim_name?: string | null;
  readonly service_principal_id?: string | null;
  readonly project_id: string;
  readonly labels?: Readonly<Record<string, string>>;
  readonly scopes?: readonly string[];
  readonly state?: IdentityMappingState;
};

export type CreateServicePrincipalCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly client_id: string;
  readonly issuer: string;
  readonly labels?: Readonly<Record<string, string>>;
  readonly scopes?: readonly string[];
  readonly allowed_project_ids?: readonly string[];
  readonly state?: ServicePrincipalState;
};

export type IdentityPrincipalType = 'user' | 'service_principal' | 'legacy_dev_token';

export type IdentityPrincipalResource = {
  readonly subject: string;
  readonly issuer: string;
  readonly display_name?: string | null;
  readonly client_id?: string | null;
  readonly principal_type: IdentityPrincipalType;
};

export type IdentityResourceCounts = {
  readonly projects: number;
  readonly service_principals: number;
  readonly identity_mappings: number;
  readonly sessions: number;
  readonly active_sessions: number;
  readonly session_templates: number;
  readonly browser_contexts: number;
  readonly egress_profiles: number;
  readonly credential_bindings: number;
  readonly file_workspaces: number;
  readonly workflow_definitions: number;
  readonly workflow_runs: number;
  readonly active_workflow_runs: number;
  readonly automation_tasks: number;
  readonly active_automation_tasks: number;
  readonly extension_definitions: number;
  readonly delegated_principals: number;
};

export type IdentityDelegatedPrincipalResource = {
  readonly client_id: string;
  readonly issuer: string;
  readonly display_name?: string | null;
  readonly registered: boolean;
  readonly registered_service_principal_id?: string | null;
  readonly state?: ServicePrincipalState | null;
  readonly session_count: number;
  readonly active_session_count: number;
  readonly session_ids: readonly string[];
};

export type IdentityServicePrincipalReviewResource = ServicePrincipalResource & {
  readonly delegated_session_count: number;
  readonly active_delegated_session_count: number;
  readonly delegated_session_ids: readonly string[];
};

export type IdentityMappingReviewResource = IdentityMappingResource & {
  readonly effective_for_principal: boolean;
};

export type IdentityUnmappedPrincipalSignalResource = {
  readonly kind: IdentityMappingKind;
  readonly issuer: string;
  readonly external_id: string;
  readonly claim_name?: string | null;
  readonly display_name?: string | null;
  readonly reason: string;
};

export type IdentityAccessReviewResponse = {
  readonly principal: IdentityPrincipalResource;
  readonly generated_at: string;
  readonly projects: readonly ProjectResource[];
  readonly resource_counts: IdentityResourceCounts;
  readonly identity_mappings: readonly IdentityMappingReviewResource[];
  readonly unmapped_principal_signals: readonly IdentityUnmappedPrincipalSignalResource[];
  readonly service_principals: readonly IdentityServicePrincipalReviewResource[];
  readonly delegated_principals: readonly IdentityDelegatedPrincipalResource[];
};

export type CreateProjectCommand = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly quotas?: ProjectQuotas;
  readonly policy?: ProjectPolicy;
  readonly state?: ProjectState;
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
  readonly project_id?: string | null;
  readonly project?: SessionProjectResource | null;
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
  readonly project_id?: string | null;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly persistence_mode?: BrowserContextPersistenceMode;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type CloneBrowserContextCommand = {
  readonly name: string;
  readonly project_id?: string | null;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly retention_sec?: number | null;
  readonly max_profile_storage_bytes?: number | null;
};

export type ImportBrowserContextCommand = {
  readonly name: string;
  readonly archive: BodyInit;
  readonly project_id?: string | null;
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

export type SessionQueueInfo = {
  readonly queued_at: string;
  readonly queued_for_ms: number;
  readonly position: number;
  readonly active_sessions: number;
  readonly queued_sessions: number;
  readonly max_active_sessions?: number | null;
  readonly dispatch_blocker: string;
  readonly cancellable: boolean;
};

export type SessionResource = {
  readonly id: string;
  readonly state: string;
  readonly project_id?: string | null;
  readonly project?: SessionProjectResource | null;
  readonly admission?: ProjectAdmissionDecision | null;
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
  readonly queue?: SessionQueueInfo | null;
  readonly created_at: string;
  readonly updated_at: string;
  readonly queued_at?: string | null;
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
  readonly project_id?: string | null;
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
  readonly project_id?: string | null;
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
  readonly project_id?: string | null;
  readonly project?: SessionProjectResource | null;
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
  readonly project_id?: string | null;
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
