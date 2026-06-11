import type { ProjectPolicy, ProjectResource } from './project-types';

export type ProjectOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly projects: readonly ProjectResource[];
      readonly selectedProjectId?: string | null;
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
  readonly activeSessions: string;
  readonly activeWorkflowRuns: string;
  readonly alerts: string;
  readonly policySummary: string;
  readonly selected: boolean;
};

export type ProjectDetailSection = {
  readonly title: string;
  readonly rows: readonly ProjectDetailRow[];
};

export type ProjectDetailRow = {
  readonly label: string;
  readonly value: string;
};

export type ProjectOverviewModel = {
  readonly metrics: readonly ProjectOverviewMetric[];
  readonly rows: readonly ProjectOverviewRow[];
  readonly selected: ProjectResource | null;
  readonly selectedSections: readonly ProjectDetailSection[];
};

export function buildProjectOverviewModel(
  projects: readonly ProjectResource[],
  selectedProjectId: string | null | undefined = null,
): ProjectOverviewModel {
  const selected = selectProject(projects, selectedProjectId);
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
    rows: projects.map((project) => projectRow(project, selected?.id ?? null)),
    selected,
    selectedSections: selected ? projectDetailSections(selected) : [],
  };
}

function selectProject(projects: readonly ProjectResource[], selectedProjectId: string | null | undefined): ProjectResource | null {
  if (projects.length === 0) {
    return null;
  }
  return projects.find((project) => project.id === selectedProjectId) ?? projects[0] ?? null;
}

function projectRow(project: ProjectResource, selectedProjectId: string | null): ProjectOverviewRow {
  return {
    id: project.id,
    name: project.name,
    description: project.description?.trim() || project.id,
    state: project.state,
    activeSessions: usageWithLimit(project.usage.active_sessions, project.usage.max_active_sessions),
    activeWorkflowRuns: usageWithLimit(project.usage.active_workflow_runs, project.usage.max_active_workflow_runs),
    alerts: usageAlertsLabel(project),
    policySummary: projectPolicySummary(project.policy),
    selected: project.id === selectedProjectId,
  };
}

function projectDetailSections(project: ProjectResource): readonly ProjectDetailSection[] {
  const labelEntries = Object.entries(project.labels);
  return [
    {
      title: 'Metadata',
      rows: [
        { label: 'Project id', value: project.id },
        { label: 'State', value: project.state },
        { label: 'Created', value: formatDateTime(project.created_at) },
        { label: 'Updated', value: formatDateTime(project.updated_at) },
        { label: 'Labels', value: labelEntries.length > 0 ? labelEntries.map(([key, value]) => `${key}=${value}`).join(', ') : 'none' },
      ],
    },
    {
      title: 'Usage',
      rows: [
        { label: 'Active sessions', value: usageWithLimit(project.usage.active_sessions, project.usage.max_active_sessions) },
        { label: 'Queued sessions', value: String(project.usage.queued_sessions) },
        {
          label: 'Active workflow runs',
          value: usageWithLimit(project.usage.active_workflow_runs, project.usage.max_active_workflow_runs),
        },
        { label: 'Session creations', value: usageWithLimit(project.usage.session_creations, project.usage.max_session_creations) },
        {
          label: 'Runtime',
          value: usageWithLimit(
            formatDuration(project.usage.runtime_usage_ms) ?? '0s',
            formatDuration(project.usage.max_runtime_usage_ms),
          ),
        },
        {
          label: 'Egress',
          value: usageWithLimit(
            formatBytes(project.usage.egress_total_bytes) ?? '0 B',
            formatBytes(project.usage.max_egress_total_bytes),
          ),
        },
        {
          label: 'Retained storage',
          value: usageWithLimit(
            formatBytes(project.usage.retained_storage_bytes) ?? '0 B',
            formatBytes(project.usage.max_retained_storage_bytes),
          ),
        },
        { label: 'Usage alerts', value: usageAlertsLabel(project) },
        { label: 'Observed', value: formatDateTime(project.usage.observed_at) },
      ],
    },
    {
      title: 'Policy',
      rows: [
        { label: 'Browser uploads', value: enabledLabel(project.policy.allow_browser_uploads) },
        { label: 'Browser downloads', value: enabledLabel(project.policy.allow_browser_downloads) },
        { label: 'Session file bindings', value: enabledLabel(project.policy.allow_session_file_bindings) },
        { label: 'Manual recordings', value: enabledLabel(project.policy.allow_manual_recordings) },
        { label: 'Budget enforcement', value: budgetEnforcementLabel(project.policy.usage_budget_enforcement) },
        { label: 'Session templates', value: restrictionLabel(project.policy.allowed_session_template_ids.length) },
        { label: 'Browser contexts', value: restrictionLabel(project.policy.allowed_browser_context_ids.length) },
        { label: 'Egress profiles', value: restrictionLabel(project.policy.allowed_egress_profile_ids.length) },
        { label: 'File workspaces', value: restrictionLabel(project.policy.allowed_file_workspace_ids.length) },
        { label: 'Extensions', value: restrictionLabel(project.policy.allowed_extension_ids.length) },
      ],
    },
  ];
}

function metric(key: string, label: string, value: number): ProjectOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `projects-metric-${key}`,
  };
}

function usageWithLimit(value: number | string, limit: number | string | null | undefined): string {
  if (limit === undefined || limit === null) {
    return `${value} / unbounded`;
  }
  return `${value} / ${limit}`;
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

function enabledLabel(enabled: boolean): string {
  return enabled ? 'enabled' : 'disabled';
}

function budgetEnforcementLabel(value: string): string {
  return value === 'block_session_creation' ? 'block session creation' : 'warning only';
}

function restrictionLabel(count: number): string {
  return count > 0 ? `${count} allowed` : 'unrestricted';
}

function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat('en', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

function formatBytes(value: number | null | undefined): string | null {
  if (value === undefined || value === null) {
    return null;
  }
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ['KB', 'MB', 'GB', 'TB'];
  let current = value / 1024;
  let unitIndex = 0;
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024;
    unitIndex += 1;
  }
  return `${current.toFixed(current >= 10 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatDuration(value: number | null | undefined): string | null {
  if (value === undefined || value === null) {
    return null;
  }
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m`;
  }
  return `${totalSeconds}s`;
}
