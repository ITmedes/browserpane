import {
  formatBytes,
  formatDateTime,
  formatDuration,
  usageWithLimit,
  type ProjectTone,
} from './project-formatters';
import type { ProjectResource, ProjectUsageResource } from './project-types';

export type ProjectInspectorRow = {
  readonly label: string;
  readonly value: string;
};

export type ProjectInspectorModel = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly identityRows: readonly ProjectInspectorRow[];
  readonly usageRows: readonly ProjectInspectorRow[];
  readonly alerts: readonly ProjectInspectorRow[];
};

export function buildProjectInspectorModel(project: ProjectResource): ProjectInspectorModel {
  return {
    id: project.id,
    name: project.name,
    description: project.description?.trim() || 'No description.',
    state: project.state,
    stateTone: project.state === 'active' ? 'success' : 'neutral',
    identityRows: identityRows(project),
    usageRows: usageRows(project.usage),
    alerts: project.usage.alerts.length > 0
      ? project.usage.alerts.map((alert) => ({
          label: `${alert.metric} ${alert.state}`,
          value: alert.message,
        }))
      : [{ label: 'Usage alerts', value: 'none' }],
  };
}

function identityRows(project: ProjectResource): readonly ProjectInspectorRow[] {
  return [
    { label: 'Project ID', value: project.id },
    { label: 'Created', value: formatDateTime(project.created_at) },
    { label: 'Updated', value: formatDateTime(project.updated_at) },
  ];
}

function usageRows(usage: ProjectUsageResource): readonly ProjectInspectorRow[] {
  return [
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
  ];
}
