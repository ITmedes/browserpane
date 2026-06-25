export type ProjectState = 'active' | 'archived';

export type ProjectUsageBudgetEnforcement = 'warning_only' | 'block_session_creation';

export type ProjectPolicyOption = {
  readonly id: string;
  readonly name: string;
  readonly description: string | null;
  readonly state: string | null;
  readonly scope: string | null;
};

export type ProjectPolicyOptions = {
  readonly sessionTemplates: readonly ProjectPolicyOption[];
  readonly browserContexts: readonly ProjectPolicyOption[];
  readonly egressProfiles: readonly ProjectPolicyOption[];
  readonly extensions: readonly ProjectPolicyOption[];
  readonly fileWorkspaces: readonly ProjectPolicyOption[];
};

export type ProjectQuotas = {
  readonly max_active_sessions?: number | null;
  readonly max_active_workflow_runs?: number | null;
  readonly max_retained_storage_bytes?: number | null;
  readonly max_session_creations?: number | null;
  readonly max_session_creations_per_window?: number | null;
  readonly session_creation_window_sec?: number | null;
  readonly max_runtime_usage_ms?: number | null;
  readonly max_egress_total_bytes?: number | null;
};

export type ProjectPolicy = {
  readonly allowed_session_template_ids: readonly string[];
  readonly allowed_egress_profile_ids: readonly string[];
  readonly allowed_extension_ids: readonly string[];
  readonly allowed_browser_context_ids: readonly string[];
  readonly allowed_file_workspace_ids: readonly string[];
  readonly allow_browser_uploads: boolean;
  readonly allow_browser_downloads: boolean;
  readonly allow_session_file_bindings: boolean;
  readonly allow_manual_recordings: boolean;
  readonly usage_budget_enforcement: ProjectUsageBudgetEnforcement;
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

export type UpsertProjectRequest = {
  readonly name: string;
  readonly description?: string | null;
  readonly labels?: Readonly<Record<string, string>>;
  readonly quotas?: ProjectQuotas;
  readonly policy?: ProjectPolicy;
  readonly state?: ProjectState;
};

export type ProjectListResponse = {
  readonly projects: readonly ProjectResource[];
};
