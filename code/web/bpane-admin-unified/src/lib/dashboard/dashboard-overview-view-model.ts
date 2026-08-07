import type { BrowserContextResource } from '$lib/browser-contexts/browser-context-types';
import type { EgressProfileResource } from '$lib/egress-profiles/egress-profile-types';
import type { FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
import type { ProjectTone } from '$lib/projects/project-formatters';
import { formatDateTime } from '$lib/projects/project-formatters';
import type { RecordingCatalogEntry } from '$lib/recordings/recording-types';
import { sessionRow } from '$lib/sessions/session-overview-view-model';
import type { SessionResource } from '$lib/sessions/session-types';
import { isActiveWorkflowRun, runNeedsAttention } from '$lib/workflow-runs/workflow-run-overview-view-model';
import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
import type { WorkflowDefinitionResource } from '$lib/workflows/workflow-types';

export type DashboardLoadFailure = {
  readonly resource: string;
  readonly message: string;
  readonly href?: string;
};

export type DashboardSnapshot = {
  readonly sessions: readonly SessionResource[];
  readonly projects: readonly ProjectResourceSummary[];
  readonly browserContexts: readonly BrowserContextResource[];
  readonly egressProfiles: readonly EgressProfileResource[];
  readonly fileWorkspaces: readonly FileWorkspaceResource[];
  readonly workflows: readonly WorkflowDefinitionResource[];
  readonly workflowRuns: readonly WorkflowRunResource[];
  readonly recordings: readonly RecordingCatalogEntry[];
};

export type ProjectResourceSummary = {
  readonly id: string;
  readonly name: string;
  readonly state: string;
  readonly usage: {
    readonly active_sessions: number;
    readonly queued_sessions: number;
    readonly active_workflow_runs: number;
    readonly alerts: readonly {
      readonly metric: string;
      readonly state: string;
      readonly message: string;
    }[];
  };
};

export type DashboardOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly snapshot: DashboardSnapshot;
      readonly failures: readonly DashboardLoadFailure[];
    };

export type DashboardMetric = {
  readonly label: string;
  readonly value: string;
  readonly detail: string;
  readonly tone: ProjectTone;
  readonly href: string;
  readonly testId: string;
};

export type DashboardQuickLink = {
  readonly label: string;
  readonly description: string;
  readonly value: string;
  readonly href: string;
  readonly testId: string;
};

export type DashboardAttentionItem = {
  readonly title: string;
  readonly description: string;
  readonly tone: ProjectTone;
  readonly href: string;
  readonly testId: string;
};

export type DashboardActivityItem = {
  readonly title: string;
  readonly detail: string;
  readonly timestamp: string;
  readonly href: string;
};

export type DashboardOverviewModel = {
  readonly metrics: readonly DashboardMetric[];
  readonly quickLinks: readonly DashboardQuickLink[];
  readonly attentionItems: readonly DashboardAttentionItem[];
  readonly recentActivity: readonly DashboardActivityItem[];
};

export function buildDashboardOverviewModel(
  snapshot: DashboardSnapshot,
  failures: readonly DashboardLoadFailure[] = [],
): DashboardOverviewModel {
  const attentionItems = buildAttentionItems(snapshot, failures);
  const activeSessions = snapshot.sessions.filter(isActiveDashboardSession).length;
  const liveClients = snapshot.sessions.reduce(
    (total, session) => total + session.status.connection_counts.total_clients,
    0,
  );
  const activeWorkflowRuns = snapshot.workflowRuns.filter(isActiveWorkflowRun).length;
  const projectAlerts = snapshot.projects.reduce((total, project) => total + project.usage.alerts.length, 0);
  const resourceCatalogCount = snapshot.browserContexts.length
    + snapshot.egressProfiles.length
    + snapshot.fileWorkspaces.length
    + snapshot.workflows.length;

  return {
    metrics: [
      metric(
        'sessions',
        'Active sessions',
        `${activeSessions}`,
        `${snapshot.sessions.length} total · ${liveClients} live clients`,
        activeSessions > 0 ? 'success' : 'neutral',
        '/admin-new/sessions',
      ),
      metric(
        'workflow-runs',
        'Workflow runs',
        `${activeWorkflowRuns}`,
        `${snapshot.workflowRuns.length} total · ${snapshot.workflowRuns.filter(runNeedsAttention).length} need attention`,
        snapshot.workflowRuns.some(runNeedsAttention) ? 'warning' : activeWorkflowRuns > 0 ? 'success' : 'neutral',
        '/admin-new/workflow-runs',
      ),
      metric(
        'project-alerts',
        'Project alerts',
        `${projectAlerts}`,
        `${snapshot.projects.filter((project) => project.state === 'active').length} active projects`,
        projectAlerts > 0 ? 'warning' : 'neutral',
        '/admin-new/projects',
      ),
      metric(
        'resources',
        'Resource catalog',
        `${resourceCatalogCount}`,
        `${snapshot.browserContexts.length} contexts · ${snapshot.egressProfiles.length} egress · ${snapshot.fileWorkspaces.length} workspaces`,
        failures.length > 0 ? 'warning' : 'neutral',
        '/admin-new/browser-contexts',
      ),
    ],
    quickLinks: [
      quickLink('Sessions', 'Start, connect, stop, and inspect browser sessions.', snapshot.sessions.length, '/admin-new/sessions', 'sessions'),
      quickLink('Workflow runs', 'Track automation execution and operator intervention.', snapshot.workflowRuns.length, '/admin-new/workflow-runs', 'runs'),
      quickLink('Recordings', 'Review retained session recording artifacts.', snapshot.recordings.length, '/admin-new/recordings', 'recordings'),
      quickLink('Projects', 'Maintain admission policy, quota, and resource bindings.', snapshot.projects.length, '/admin-new/projects', 'projects'),
      quickLink('Browser contexts', 'Manage reusable and ephemeral browser state resources.', snapshot.browserContexts.length, '/admin-new/browser-contexts', 'contexts'),
      quickLink('Egress profiles', 'Configure direct, proxy, and TLS-intercept routing.', snapshot.egressProfiles.length, '/admin-new/egress', 'egress'),
      quickLink('File workspaces', 'Maintain file repositories available to sessions and workflows.', snapshot.fileWorkspaces.length, '/admin-new/files/workspaces', 'workspaces'),
      quickLink('Workflows', 'Review workflow definitions and source metadata.', snapshot.workflows.length, '/admin-new/workflows', 'workflows'),
    ],
    attentionItems,
    recentActivity: recentActivity(snapshot),
  };
}

function buildAttentionItems(
  snapshot: DashboardSnapshot,
  failures: readonly DashboardLoadFailure[],
): readonly DashboardAttentionItem[] {
  return [
    ...failures.map((failure, index) => ({
      title: `${failure.resource} unavailable`,
      description: failure.message,
      tone: 'warning' as ProjectTone,
      href: failure.href ?? '/admin-new/',
      testId: `dashboard-attention-failure-${index}`,
    })),
    ...snapshot.projects.flatMap((project) =>
      project.usage.alerts.map((alert) => ({
        title: `${project.name} ${alert.metric.replaceAll('_', ' ')}`,
        description: alert.message,
        tone: alert.state === 'exceeded' ? 'danger' as ProjectTone : 'warning' as ProjectTone,
        href: `/admin-new/projects/${encodeURIComponent(project.id)}`,
        testId: `dashboard-attention-project-${project.id}-${alert.metric}`,
      })),
    ),
    ...snapshot.sessions
      .map((session) => ({ session, row: sessionRow(session) }))
      .filter(({ row }) => row.attention !== 'none')
      .map(({ session, row }) => ({
        title: `Session ${row.shortId}`,
        description: row.attention,
        tone: row.attentionTone,
        href: `/admin-new/sessions/${encodeURIComponent(session.id)}`,
        testId: `dashboard-attention-session-${session.id}`,
      })),
    ...snapshot.workflowRuns.filter(runNeedsAttention).map((run) => ({
      title: `Workflow run ${shortIdentifier(run.id)}`,
      description: workflowRunAttentionDescription(run),
      tone: run.state === 'failed' || run.state === 'timed_out' ? 'danger' as ProjectTone : 'warning' as ProjectTone,
      href: '/admin-new/workflow-runs',
      testId: `dashboard-attention-run-${run.id}`,
    })),
    ...snapshot.recordings
      .filter((entry) => entry.recording.state === 'failed' || (entry.recording.state === 'ready' && !entry.recording.artifact_available))
      .map((entry) => ({
        title: `Recording ${shortIdentifier(entry.recording.id)}`,
        description: entry.recording.error?.trim()
          || (entry.recording.artifact_available ? entry.recording.state : 'artifact unavailable'),
        tone: entry.recording.state === 'failed' ? 'danger' as ProjectTone : 'warning' as ProjectTone,
        href: '/admin-new/recordings',
        testId: `dashboard-attention-recording-${entry.recording.id}`,
      })),
    ...snapshot.browserContexts
      .filter((context) => context.usage.profile_storage_limit_exceeded || context.state !== 'ready')
      .map((context) => ({
        title: context.name,
        description: context.usage.profile_storage_limit_exceeded
          ? 'Profile storage limit exceeded'
          : `Browser context is ${context.state}`,
        tone: context.state === 'deleted' ? 'neutral' as ProjectTone : 'warning' as ProjectTone,
        href: `/admin-new/browser-contexts/${encodeURIComponent(context.id)}`,
        testId: `dashboard-attention-context-${context.id}`,
      })),
    ...snapshot.egressProfiles
      .filter((profile) => profile.diagnostics.health !== 'ready')
      .map((profile) => ({
        title: profile.name,
        description: `Egress diagnostics ${profile.diagnostics.health}`,
        tone: profile.diagnostics.health === 'blocked' ? 'danger' as ProjectTone : 'warning' as ProjectTone,
        href: `/admin-new/egress/${encodeURIComponent(profile.id)}`,
        testId: `dashboard-attention-egress-${profile.id}`,
      })),
  ].slice(0, 12);
}

function recentActivity(snapshot: DashboardSnapshot): readonly DashboardActivityItem[] {
  return [
    ...snapshot.sessions.map((session) => ({
      title: `Session ${session.labels.name ?? shortIdentifier(session.id)}`,
      detail: `${session.state} · ${session.status.connection_counts.total_clients} clients`,
      timestamp: session.updated_at,
      href: `/admin-new/sessions/${encodeURIComponent(session.id)}`,
    })),
    ...snapshot.workflowRuns.map((run) => ({
      title: `Run ${shortIdentifier(run.id)}`,
      detail: `${run.workflow_definition_id} · ${run.state}`,
      timestamp: run.updated_at,
      href: '/admin-new/workflow-runs',
    })),
    ...snapshot.recordings.map((entry) => ({
      title: `Recording ${shortIdentifier(entry.recording.id)}`,
      detail: `${entry.session.labels.name ?? shortIdentifier(entry.session.id)} · ${entry.recording.state}`,
      timestamp: entry.recording.updated_at,
      href: '/admin-new/recordings',
    })),
  ]
    .sort((left, right) => new Date(right.timestamp).getTime() - new Date(left.timestamp).getTime())
    .slice(0, 6)
    .map((item) => ({
      ...item,
      timestamp: formatDateTime(item.timestamp),
    }));
}

function workflowRunAttentionDescription(run: WorkflowRunResource): string {
  if (run.intervention.pending_request) {
    return run.intervention.pending_request.prompt ?? `${run.intervention.pending_request.kind} requested`;
  }
  if (run.error?.trim()) {
    return run.error.trim();
  }
  if (run.admission?.state === 'queued') {
    return `Queued: ${run.admission.reason}`;
  }
  if (run.project_admission?.state === 'denied') {
    return run.project_admission.message;
  }
  return run.state.replaceAll('_', ' ');
}

function isActiveDashboardSession(session: SessionResource): boolean {
  return !['stopped', 'released', 'killed', 'cancelled', 'failed'].includes(session.state)
    && session.status.runtime_state !== 'stopped';
}

function shortIdentifier(value: string): string {
  return value.length <= 12 ? value : value.slice(0, 12);
}

function metric(
  key: string,
  label: string,
  value: string,
  detail: string,
  tone: ProjectTone,
  href: string,
): DashboardMetric {
  return {
    label,
    value,
    detail,
    tone,
    href,
    testId: `dashboard-metric-${key}`,
  };
}

function quickLink(
  label: string,
  description: string,
  count: number,
  href: string,
  key: string,
): DashboardQuickLink {
  return {
    label,
    description,
    value: String(count),
    href,
    testId: `dashboard-link-${key}`,
  };
}
