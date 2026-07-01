import {
  formatDateTime,
  formatDuration,
} from '$lib/projects/project-formatters';
import type { ProjectTone } from '$lib/projects/project-formatters';
import type { SessionResource, SessionStatus } from './session-types';
import { sessionRow } from './session-overview-view-model';

export type SessionDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly sessionId: string }
  | { readonly status: 'error'; readonly sessionId: string; readonly message: string }
  | { readonly status: 'ready'; readonly session: SessionResource; readonly liveStatus: SessionStatus | null };

export type SessionFact = {
  readonly label: string;
  readonly value: string;
  readonly testId?: string;
  readonly tone?: ProjectTone;
};

export type SessionFactSection = {
  readonly title: string;
  readonly facts: readonly SessionFact[];
  readonly testId: string;
};

export type SessionDetailActionModel = {
  readonly canConnectPreview: boolean;
  readonly connectPreviewLabel: string;
  readonly connectPreviewDescription: string;
  readonly canCancelQueue: boolean;
  readonly canDisconnectAll: boolean;
  readonly canRelease: boolean;
  readonly canStop: boolean;
  readonly canKill: boolean;
};

export type SessionDetailModel = {
  readonly id: string;
  readonly shortId: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly subtitle: string;
  readonly sections: readonly SessionFactSection[];
  readonly connections: readonly { readonly id: number; readonly role: string }[];
  readonly actions: SessionDetailActionModel;
};

export function buildSessionDetailModel(session: SessionResource, liveStatus: SessionStatus | null): SessionDetailModel {
  const row = sessionRow(session);
  const status = liveStatus ?? null;
  const connectionCounts = status?.connection_counts ?? session.status.connection_counts;
  const stopEligibility = status?.stop_eligibility ?? session.status.stop_eligibility;
  const totalClients = connectionCounts.total_clients;
  const isQueued = session.state === 'queued';
  const terminal = ['stopped', 'killed', 'cancelled', 'failed'].includes(session.state);
  return {
    id: session.id,
    shortId: row.shortId,
    state: session.state,
    stateTone: row.stateTone,
    subtitle: `${row.project} | ${row.browserContext} | ${row.egress}`,
    sections: [
      {
        title: 'Lifecycle',
        testId: 'session-detail-lifecycle',
        facts: [
          fact('State', session.state, 'session-detail-lifecycle-state', row.stateTone),
          fact('Runtime', status?.runtime_state ?? session.status.runtime_state, 'session-detail-runtime'),
          fact('Resume mode', status?.runtime_resume_mode ?? session.status.runtime_resume_mode),
          fact('Presence', status?.presence_state ?? session.status.presence_state),
          fact('Stop eligibility', stopEligibility.allowed ? 'allowed' : blockersLabel(stopEligibility.blockers), 'session-detail-stop-eligibility'),
          fact('Created', formatDateTime(session.created_at)),
          fact('Updated', formatDateTime(session.updated_at)),
          ...(session.queued_at ? [fact('Queued', formatDateTime(session.queued_at))] : []),
          ...(session.runtime_released_at ? [fact('Runtime released', formatDateTime(session.runtime_released_at))] : []),
          ...(session.stopped_at ? [fact('Stopped', formatDateTime(session.stopped_at))] : []),
        ],
      },
      {
        title: 'Scope',
        testId: 'session-detail-scope',
        facts: [
          fact('Project', session.project?.name ?? session.project_id ?? 'owner scope', 'session-detail-project'),
          fact('Admission', session.admission ? `${session.admission.state}: ${session.admission.reason_code}` : 'owner scope'),
          fact('Template', session.template_id ?? 'none', 'session-detail-template'),
          fact('Owner mode', session.owner_mode),
          fact('Automation delegate', session.automation_delegate?.display_name ?? session.automation_delegate?.client_id ?? 'none'),
          fact('Labels', labelSummary(session.labels)),
          fact('Integration', integrationSummary(session.integration_context ?? null)),
        ],
      },
      {
        title: 'Runtime',
        testId: 'session-detail-runtime-section',
        facts: [
          fact('Binding', session.runtime.binding || 'unassigned'),
          fact('Runtime mode', session.runtime.compatibility_mode || 'unknown'),
          fact('Transport', session.connect.compatibility_mode || 'unknown'),
          fact('CDP endpoint', session.runtime.cdp_endpoint ?? 'none'),
          fact('Viewport', session.viewport ? `${session.viewport.width} x ${session.viewport.height}` : 'default'),
          fact('Resolution', status ? `${status.resolution[0]} x ${status.resolution[1]}` : 'not loaded'),
          fact('Idle timeout', session.idle_timeout_sec ? formatDuration(session.idle_timeout_sec * 1000) ?? `${session.idle_timeout_sec}s` : 'default'),
          fact('Recording', recordingLabel(session), 'session-detail-recording', session.recording.mode === 'disabled' ? 'neutral' : 'success'),
        ],
      },
      {
        title: 'Connections',
        testId: 'session-detail-connections',
        facts: [
          fact('Total', String(totalClients), 'session-detail-total-clients'),
          fact('Interactive', String(connectionCounts.interactive_clients)),
          fact('Owners', String(connectionCounts.owner_clients)),
          fact('Viewers', String(connectionCounts.viewer_clients)),
          fact('Recorders', String(connectionCounts.recorder_clients)),
          fact('Automation', String(connectionCounts.automation_clients)),
          fact('MCP owner', status?.mcp_owner ? 'active' : 'inactive'),
        ],
      },
      {
        title: 'Browser Context And Egress',
        testId: 'session-detail-egress',
        facts: [
          fact('Browser context', row.browserContext, 'session-detail-browser-context'),
          fact('Locale', session.network_identity?.locale ?? 'default'),
          fact('Timezone', session.network_identity?.timezone ?? 'default'),
          fact('Egress', row.egress, 'session-detail-egress-label'),
          fact('Proxy', session.effective_egress?.proxy_configured ? 'configured' : 'direct'),
          fact('Observation', session.effective_egress?.observation_mode ?? 'metadata_only'),
          fact('TLS interception', session.effective_egress?.tls_interception_enabled ? 'enabled' : 'disabled'),
          fact('Diagnostics', session.egress_diagnostics ? `${session.egress_diagnostics.health} / ${session.egress_diagnostics.proof_level}` : 'not collected'),
        ],
      },
      {
        title: 'Capabilities',
        testId: 'session-detail-capabilities',
        facts: Object.entries(session.capabilities).map(([key, enabled]) =>
          fact(key.replace(/_/g, ' '), enabled ? 'enabled' : 'disabled', `session-capability-${key}`, enabled ? 'success' : 'neutral'),
        ),
      },
      ...(session.queue ? [{
        title: 'Queue',
        testId: 'session-detail-queue',
        facts: [
          fact('Position', `${session.queue.position}/${session.queue.queued_sessions}`, 'session-detail-queue-position'),
          fact('Queued for', formatDuration(session.queue.queued_for_ms) ?? '0s'),
          fact('Active sessions', limitLabel(session.queue.active_sessions, session.queue.max_active_sessions)),
          fact('Blocker', session.queue.dispatch_blocker),
          fact('Cancellable', session.queue.cancellable ? 'yes' : 'no'),
        ],
      }] : []),
    ],
    connections: status?.connections.map((connection) => ({
      id: connection.connection_id,
      role: connection.role,
    })) ?? [],
    actions: {
      canConnectPreview: !isQueued && !['killed', 'cancelled', 'failed'].includes(session.state),
      connectPreviewLabel: connectPreviewLabel(session.state),
      connectPreviewDescription: connectPreviewDescription(session.state),
      canCancelQueue: isQueued && (session.queue?.cancellable ?? true),
      canDisconnectAll: totalClients > 0,
      canRelease: !isQueued && !terminal && totalClients === 0 && stopEligibility.allowed && session.state !== 'released',
      canStop: !isQueued && !terminal && totalClients === 0 && stopEligibility.allowed,
      canKill: !isQueued && !terminal,
    },
  };
}

function connectPreviewLabel(state: string): string {
  if (state === 'stopped' || state === 'released') {
    return 'Start and connect';
  }
  return 'Connect';
}

function connectPreviewDescription(state: string): string {
  if (state === 'stopped' || state === 'released') {
    return 'Start a runtime from the persisted browser profile and open the browser preview in a popup window.';
  }
  return 'Open this browser session in a popup preview window.';
}

function fact(label: string, value: string, testId?: string, tone?: ProjectTone): SessionFact {
  return {
    label,
    value,
    ...(testId ? { testId } : {}),
    ...(tone ? { tone } : {}),
  };
}

function blockersLabel(blockers: readonly { readonly kind: string; readonly count: number }[]): string {
  if (blockers.length === 0) {
    return 'blocked';
  }
  return blockers.map((blocker) => `${blocker.count} ${blocker.kind}`).join(', ');
}

function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels);
  if (entries.length === 0) {
    return 'none';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function integrationSummary(value: Readonly<Record<string, unknown>> | null): string {
  if (!value || Object.keys(value).length === 0) {
    return 'none';
  }
  return Object.keys(value).join(', ');
}

function recordingLabel(session: SessionResource): string {
  const mode = session.recording.mode || 'disabled';
  if (mode === 'disabled') {
    return 'disabled';
  }
  const format = session.recording.format || 'webm';
  const retention = session.recording.retention_sec
    ? `, retention ${formatDuration(session.recording.retention_sec * 1000) ?? `${session.recording.retention_sec}s`}`
    : '';
  return `${mode} / ${format}${retention}`;
}

function limitLabel(value: number, limit?: number | null): string {
  if (limit === null || limit === undefined) {
    return String(value);
  }
  return `${value} / ${limit}`;
}
