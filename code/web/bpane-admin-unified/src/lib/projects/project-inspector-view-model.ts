import {
  formatBytes,
  formatDateTime,
  formatDuration,
  usageWithLimit,
  type ProjectTone,
} from './project-formatters';
import type { ProjectPolicy, ProjectQuotas, ProjectResource, ProjectUsageResource } from './project-types';

export type ProjectInspectorRow = {
  readonly label: string;
  readonly value: string;
};

export type ProjectInspectorSection = {
  readonly title: string;
  readonly rows: readonly ProjectInspectorRow[];
};

export type ProjectInspectorModel = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly sections: readonly ProjectInspectorSection[];
  readonly alerts: readonly ProjectInspectorRow[];
};

export function buildProjectInspectorModel(project: ProjectResource): ProjectInspectorModel {
  return {
    id: project.id,
    name: project.name,
    description: project.description?.trim() || 'No description.',
    state: project.state,
    stateTone: project.state === 'active' ? 'success' : 'neutral',
    sections: [
      metadataSection(project),
      usageSection(project.usage),
      quotaSection(project.quotas),
      policySection(project.policy),
    ],
    alerts: project.usage.alerts.length > 0
      ? project.usage.alerts.map((alert) => ({
          label: `${alert.metric} ${alert.state}`,
          value: alert.message,
        }))
      : [{ label: 'Usage alerts', value: 'none' }],
  };
}

function metadataSection(project: ProjectResource): ProjectInspectorSection {
  return {
    title: 'Metadata',
    rows: [
      { label: 'Project ID', value: project.id },
      { label: 'State', value: project.state },
      { label: 'Created', value: formatDateTime(project.created_at) },
      { label: 'Updated', value: formatDateTime(project.updated_at) },
      { label: 'Labels', value: formatLabels(project.labels) },
    ],
  };
}

function usageSection(usage: ProjectUsageResource): ProjectInspectorSection {
  return {
    title: 'Usage',
    rows: [
      { label: 'Active sessions', value: usageWithLimit(usage.active_sessions, usage.max_active_sessions) },
      { label: 'Queued sessions', value: String(usage.queued_sessions) },
      { label: 'Session creations', value: usageWithLimit(usage.session_creations, usage.max_session_creations) },
      { label: 'Active workflow runs', value: usageWithLimit(usage.active_workflow_runs, usage.max_active_workflow_runs) },
      {
        label: 'Runtime',
        value: usageWithLimit(formatDuration(usage.runtime_usage_ms) ?? '0s', formatDuration(usage.max_runtime_usage_ms)),
      },
      {
        label: 'Egress total',
        value: usageWithLimit(formatBytes(usage.egress_total_bytes) ?? '0 B', formatBytes(usage.max_egress_total_bytes)),
      },
      { label: 'Egress RX', value: formatBytes(usage.egress_rx_bytes) ?? '0 B' },
      { label: 'Egress TX', value: formatBytes(usage.egress_tx_bytes) ?? '0 B' },
      {
        label: 'Retained storage',
        value: usageWithLimit(
          formatBytes(usage.retained_storage_bytes) ?? '0 B',
          formatBytes(usage.max_retained_storage_bytes),
        ),
      },
      { label: 'Observed', value: formatDateTime(usage.observed_at) },
    ],
  };
}

function quotaSection(quotas: ProjectQuotas): ProjectInspectorSection {
  return {
    title: 'Quotas',
    rows: [
      { label: 'Max active sessions', value: quotaValue(quotas.max_active_sessions) },
      { label: 'Max active workflow runs', value: quotaValue(quotas.max_active_workflow_runs) },
      { label: 'Max session creations', value: quotaValue(quotas.max_session_creations) },
      { label: 'Session creation window', value: secondsValue(quotas.session_creation_window_sec) },
      { label: 'Max creations/window', value: quotaValue(quotas.max_session_creations_per_window) },
      { label: 'Max runtime', value: formatDuration(quotas.max_runtime_usage_ms) ?? 'unbounded' },
      { label: 'Max egress', value: formatBytes(quotas.max_egress_total_bytes) ?? 'unbounded' },
      { label: 'Max retained storage', value: formatBytes(quotas.max_retained_storage_bytes) ?? 'unbounded' },
    ],
  };
}

function policySection(policy: ProjectPolicy): ProjectInspectorSection {
  return {
    title: 'Policy',
    rows: [
      { label: 'Browser uploads', value: enabledValue(policy.allow_browser_uploads) },
      { label: 'Browser downloads', value: enabledValue(policy.allow_browser_downloads) },
      { label: 'Session file bindings', value: enabledValue(policy.allow_session_file_bindings) },
      { label: 'Manual recordings', value: enabledValue(policy.allow_manual_recordings) },
      { label: 'Budget enforcement', value: policy.usage_budget_enforcement },
      { label: 'Session templates', value: restrictionValue(policy.allowed_session_template_ids.length) },
      { label: 'Browser contexts', value: restrictionValue(policy.allowed_browser_context_ids.length) },
      { label: 'Egress profiles', value: restrictionValue(policy.allowed_egress_profile_ids.length) },
      { label: 'File workspaces', value: restrictionValue(policy.allowed_file_workspace_ids.length) },
      { label: 'Extensions', value: restrictionValue(policy.allowed_extension_ids.length) },
    ],
  };
}

function formatLabels(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels);
  if (entries.length === 0) {
    return 'none';
  }
  return entries
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}=${value}`)
    .join(', ');
}

function quotaValue(value: number | null | undefined): string {
  return value === undefined || value === null ? 'unbounded' : String(value);
}

function secondsValue(value: number | null | undefined): string {
  return value === undefined || value === null ? 'not configured' : `${value}s`;
}

function enabledValue(value: boolean): string {
  return value ? 'enabled' : 'disabled';
}

function restrictionValue(count: number): string {
  return count > 0 ? `${count} allowed` : 'unrestricted';
}
