import type { ProjectPolicy, ProjectResource } from './project-types';
import {
  formatBytes,
  formatDateTime,
  formatDuration,
  usageWithLimit,
  type ProjectTone,
} from './project-formatters';

export type ProjectOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly projects: readonly ProjectResource[];
    };

export type ProjectOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type ProjectOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly activeSessions: string;
  readonly queuedSessions: string;
  readonly activeWorkflowRuns: string;
  readonly sessionCreations: string;
  readonly runtimeUsage: string;
  readonly egressUsage: string;
  readonly retainedStorage: string;
  readonly alerts: string;
  readonly alertTone: ProjectTone;
  readonly policySummary: string;
  readonly updatedAt: string;
};

export type ProjectOverviewModel = {
  readonly metrics: readonly ProjectOverviewMetric[];
  readonly rows: readonly ProjectOverviewRow[];
};

export function buildProjectOverviewModel(
  projects: readonly ProjectResource[],
): ProjectOverviewModel {
  return {
    metrics: [
      metric('total', 'Projects', projects.length),
      metric('active', 'Active', projects.filter((project) => project.state === 'active').length),
      metric('archived', 'Archived', projects.filter((project) => project.state === 'archived').length),
      metric(
        'alerts',
        'Usage alerts',
        projects.reduce((total, project) => total + project.usage.alerts.length, 0),
      ),
    ],
    rows: projects.map(projectRow),
  };
}

function projectRow(project: ProjectResource): ProjectOverviewRow {
  return {
    id: project.id,
    name: project.name,
    description: project.description?.trim() || project.id,
    state: project.state,
    stateTone: project.state === 'active' ? 'success' : 'neutral',
    activeSessions: usageWithLimit(project.usage.active_sessions, project.usage.max_active_sessions),
    queuedSessions: String(project.usage.queued_sessions),
    activeWorkflowRuns: usageWithLimit(project.usage.active_workflow_runs, project.usage.max_active_workflow_runs),
    sessionCreations: usageWithLimit(project.usage.session_creations, project.usage.max_session_creations),
    runtimeUsage: usageWithLimit(
      formatDuration(project.usage.runtime_usage_ms) ?? '0s',
      formatDuration(project.usage.max_runtime_usage_ms),
    ),
    egressUsage: usageWithLimit(
      formatBytes(project.usage.egress_total_bytes) ?? '0 B',
      formatBytes(project.usage.max_egress_total_bytes),
    ),
    retainedStorage: usageWithLimit(
      formatBytes(project.usage.retained_storage_bytes) ?? '0 B',
      formatBytes(project.usage.max_retained_storage_bytes),
    ),
    alerts: usageAlertsLabel(project),
    alertTone: usageAlertTone(project),
    policySummary: projectPolicySummary(project.policy),
    updatedAt: formatDateTime(project.updated_at),
  };
}

function metric(key: string, label: string, value: number): ProjectOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `projects-metric-${key}`,
  };
}

function usageAlertsLabel(project: ProjectResource): string {
  if (project.usage.alerts.length === 0) {
    return 'none';
  }
  const exceeded = project.usage.alerts.filter((alert) => alert.state === 'exceeded').length;
  const approaching = project.usage.alerts.length - exceeded;
  return [
    exceeded > 0 ? `${exceeded} exceeded` : null,
    approaching > 0 ? `${approaching} warning` : null,
  ].filter(Boolean).join(', ');
}

function usageAlertTone(project: ProjectResource): ProjectTone {
  if (project.usage.alerts.some((alert) => alert.state === 'exceeded')) {
    return 'danger';
  }
  if (project.usage.alerts.length > 0) {
    return 'warning';
  }
  return 'neutral';
}

function projectPolicySummary(policy: ProjectPolicy): string {
  const restrictions = [
    policy.allowed_session_template_ids.length,
    policy.allowed_egress_profile_ids.length,
    policy.allowed_extension_ids.length,
    policy.allowed_browser_context_ids.length,
    policy.allowed_file_workspace_ids.length,
  ].filter((count) => count > 0).length;
  const disabledTransfers = [
    !policy.allow_browser_uploads,
    !policy.allow_browser_downloads,
    !policy.allow_session_file_bindings,
    !policy.allow_manual_recordings,
  ].filter(Boolean).length;

  if (restrictions === 0 && disabledTransfers === 0 && policy.usage_budget_enforcement === 'warning_only') {
    return 'unrestricted, warning budgets';
  }

  return [
    restrictions > 0 ? `${restrictions} restricted selectors` : null,
    disabledTransfers > 0 ? `${disabledTransfers} disabled operations` : null,
    policy.usage_budget_enforcement === 'block_session_creation' ? 'blocking budgets' : 'warning budgets',
  ].filter(Boolean).join(', ');
}
