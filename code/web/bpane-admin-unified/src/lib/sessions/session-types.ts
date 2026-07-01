export type SessionProjectResource = {
  readonly id: string;
  readonly name: string;
  readonly state?: string | null;
};

export type ProjectAdmissionDecision = {
  readonly state: string;
  readonly reason_code: string;
  readonly message: string;
  readonly checked_at: string;
};

export type SessionViewport = {
  readonly width: number;
  readonly height: number;
};

export type SessionConnectionCounts = {
  readonly interactive_clients: number;
  readonly owner_clients: number;
  readonly viewer_clients: number;
  readonly recorder_clients: number;
  readonly automation_clients: number;
  readonly total_clients: number;
};

export type SessionStopEligibility = {
  readonly allowed: boolean;
  readonly blockers: readonly {
    readonly kind: string;
    readonly count: number;
  }[];
};

export type SessionStatusSummary = {
  readonly runtime_state: string;
  readonly runtime_resume_mode: string;
  readonly presence_state: string;
  readonly connection_counts: SessionConnectionCounts;
  readonly stop_eligibility: SessionStopEligibility;
};

export type SessionBrowserContext = {
  readonly mode: string;
  readonly context_id?: string | null;
};

export type SessionNetworkIdentity = {
  readonly locale?: string | null;
  readonly languages?: readonly string[];
  readonly timezone?: string | null;
  readonly user_agent?: string | null;
  readonly browser_identity?: string | null;
  readonly egress_profile_id?: string | null;
};

export type SessionEffectiveEgress = {
  readonly profile_id?: string | null;
  readonly profile_name?: string | null;
  readonly profile_state?: string | null;
  readonly proxy_configured: boolean;
  readonly proxy_auth_configured: boolean;
  readonly bypass_rule_count: number;
  readonly custom_ca_configured: boolean;
  readonly observation_mode: string;
  readonly tls_interception_enabled: boolean;
  readonly sensitive_log_sink_configured: boolean;
};

export type SessionEgressDiagnostics = {
  readonly health: string;
  readonly proof_level: string;
  readonly observation_mode: string;
  readonly warnings: readonly string[];
  readonly observed_at: string;
};

export type SessionCapabilities = {
  readonly browser_input: boolean;
  readonly clipboard: boolean;
  readonly audio: boolean;
  readonly microphone: boolean;
  readonly camera: boolean;
  readonly file_transfer: boolean;
  readonly resize: boolean;
};

export type SessionRecordingPolicy = {
  readonly mode: 'disabled' | 'manual' | 'always' | string;
  readonly format: 'webm' | string;
  readonly retention_sec?: number | null;
};

export type SessionAutomationDelegate = {
  readonly client_id: string;
  readonly issuer: string;
  readonly display_name?: string | null;
};

export type SessionConnectInfo = {
  readonly gateway_url: string;
  readonly transport_path: string;
  readonly auth_type: string;
  readonly ticket_path?: string | null;
  readonly compatibility_mode: string;
};

export type SessionRuntimeInfo = {
  readonly binding: string;
  readonly compatibility_mode: string;
  readonly cdp_endpoint?: string | null;
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
  readonly browser_context: SessionBrowserContext;
  readonly network_identity?: SessionNetworkIdentity | null;
  readonly effective_egress?: SessionEffectiveEgress | null;
  readonly egress_diagnostics?: SessionEgressDiagnostics | null;
  readonly owner_mode: string;
  readonly viewport?: SessionViewport | null;
  readonly capabilities: SessionCapabilities;
  readonly automation_delegate?: SessionAutomationDelegate | null;
  readonly idle_timeout_sec?: number | null;
  readonly labels: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
  readonly recording: SessionRecordingPolicy;
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

export type SessionAccessTokenResponse = {
  readonly session_id: string;
  readonly token_type: string;
  readonly token: string;
  readonly expires_at: string;
  readonly connect: SessionConnectInfo;
};

export type CreateSessionRequest = {
  readonly project_id?: string | null;
  readonly template_id?: string | null;
  readonly browser_context?: SessionBrowserContext;
  readonly network_identity?: SessionNetworkIdentity | null;
  readonly owner_mode?: string | null;
  readonly viewport?: SessionViewport | null;
  readonly capabilities?: SessionCapabilities;
  readonly idle_timeout_sec?: number | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly integration_context?: Readonly<Record<string, unknown>> | null;
  readonly recording?: SessionRecordingPolicy;
};

export type SessionConnectionInfo = {
  readonly connection_id: number;
  readonly role: string;
};

export type SessionIdleStatus = {
  readonly idle_timeout_sec?: number | null;
  readonly idle_since?: string | null;
  readonly idle_deadline?: string | null;
};

export type SessionRecordingStatus = {
  readonly configured_mode: string;
  readonly format: string;
  readonly retention_sec?: number | null;
  readonly state: string;
  readonly active_recording_id?: string | null;
  readonly recorder_attached: boolean;
  readonly started_at?: string | null;
  readonly bytes_written?: number | null;
  readonly duration_ms?: number | null;
};

export type SessionStatus = {
  readonly state: string;
  readonly project_id?: string | null;
  readonly project?: SessionProjectResource | null;
  readonly admission?: ProjectAdmissionDecision | null;
  readonly runtime_state: string;
  readonly runtime_resume_mode: string;
  readonly presence_state: string;
  readonly connection_counts: SessionConnectionCounts;
  readonly stop_eligibility: SessionStopEligibility;
  readonly idle: SessionIdleStatus;
  readonly connections: readonly SessionConnectionInfo[];
  readonly browser_clients: number;
  readonly viewer_clients: number;
  readonly recorder_clients: number;
  readonly max_viewers: number;
  readonly viewer_slots_remaining: number;
  readonly exclusive_browser_owner: boolean;
  readonly mcp_owner: boolean;
  readonly resolution: readonly [number, number];
  readonly network_identity?: SessionNetworkIdentity | null;
  readonly effective_egress?: SessionEffectiveEgress | null;
  readonly egress_diagnostics?: SessionEgressDiagnostics | null;
  readonly recording?: SessionRecordingStatus | null;
};
