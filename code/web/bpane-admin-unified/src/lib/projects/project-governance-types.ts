import type { ProjectTone } from './project-formatters';
import type { ProjectUsageAlertResource } from './project-types';

export type ProjectResourceKind =
  | 'session_template'
  | 'browser_context'
  | 'egress_profile'
  | 'extension'
  | 'file_workspace';

export type ProjectResourceDecisionCode =
  | 'allowed_unrestricted'
  | 'allowed_by_policy'
  | 'blocked_by_policy'
  | 'blocked_project_scope'
  | 'blocked_resource_state'
  | 'missing_reference';

export type ProjectResourceDecision = {
  readonly id: string;
  readonly kind: ProjectResourceKind;
  readonly name: string;
  readonly allowed: boolean;
  readonly code: ProjectResourceDecisionCode;
  readonly reason: string;
  readonly tone: ProjectTone;
};

export type ProjectAllowlistSummary = {
  readonly kind: ProjectResourceKind;
  readonly label: string;
  readonly mode: 'unrestricted' | 'restricted';
  readonly configuredCount: number;
  readonly resources: readonly ProjectResourceDecision[];
};

export type ProjectOperationPolicyId =
  | 'browser_uploads'
  | 'browser_downloads'
  | 'session_file_bindings'
  | 'manual_recordings';

export type ProjectOperationPolicy = {
  readonly id: ProjectOperationPolicyId;
  readonly label: string;
  readonly allowed: boolean;
  readonly reason: string;
  readonly tone: ProjectTone;
};

export type ProjectUsageMetricId =
  | 'active_sessions'
  | 'queued_sessions'
  | 'active_workflow_runs'
  | 'session_creations'
  | 'runtime_usage_ms'
  | 'egress_total_bytes'
  | 'retained_storage_bytes';

export type ProjectUsagePressureState =
  | 'unbounded'
  | 'normal'
  | 'approaching'
  | 'at_limit'
  | 'exceeded';

export type ProjectUsagePressure = {
  readonly id: ProjectUsageMetricId;
  readonly label: string;
  readonly current: number;
  readonly limit: number | null;
  readonly displayValue: string;
  readonly state: ProjectUsagePressureState;
  readonly tone: ProjectTone;
  readonly description: string;
};

export type ProjectUsageGovernanceModel = {
  readonly enforcement: 'warning_only' | 'block_session_creation';
  readonly enforcementLabel: string;
  readonly metrics: readonly ProjectUsagePressure[];
  readonly egressReceiveLabel: string;
  readonly egressTransmitLabel: string;
  readonly alerts: readonly ProjectUsageAlertResource[];
  readonly observedAt: string;
};

export type ProjectRelatedWorkKind = 'session' | 'workflow_run';

export type ProjectRelatedWorkItem = {
  readonly kind: ProjectRelatedWorkKind;
  readonly id: string;
  readonly state: string;
  readonly href: string;
  readonly admissionState: string | null;
  readonly reasonCode: string | null;
  readonly message: string | null;
  readonly queuedAt: string | null;
  readonly queuePosition: number | null;
  readonly updatedAt: string;
  readonly tone: ProjectTone;
};

export type ProjectRelatedWorkModel = {
  readonly sessions: readonly ProjectRelatedWorkItem[];
  readonly workflowRuns: readonly ProjectRelatedWorkItem[];
  readonly queuedSessions: number;
  readonly queuedWorkflowRuns: number;
};
