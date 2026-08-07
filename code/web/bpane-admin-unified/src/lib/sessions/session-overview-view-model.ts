import type { AdminActionState } from '$lib/application/admin-async-state';
import { formatDateTime } from '$lib/projects/project-formatters';
import type { ProjectTone } from '$lib/projects/project-formatters';
import type { SessionResource } from './session-types';

export type SessionOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly sessions: readonly SessionResource[] };

export type SessionActionState = AdminActionState;

export type SessionOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type SessionOverviewRow = {
  readonly id: string;
  readonly shortId: string;
  readonly state: string;
  readonly stateTone: ProjectTone;
  readonly runtime: string;
  readonly presence: string;
  readonly clients: string;
  readonly project: string;
  readonly admission: string;
  readonly template: string;
  readonly browserContext: string;
  readonly egress: string;
  readonly capabilities: string;
  readonly attention: string;
  readonly attentionTone: ProjectTone;
  readonly updatedAt: string;
};

export type SessionOverviewModel = {
  readonly metrics: readonly SessionOverviewMetric[];
  readonly rows: readonly SessionOverviewRow[];
};

export function buildSessionOverviewModel(sessions: readonly SessionResource[]): SessionOverviewModel {
  return {
    metrics: [
      metric('total', 'Sessions', sessions.length),
      metric('active', 'Active', sessions.filter(isActiveSession).length),
      metric('queued', 'Queued', sessions.filter((session) => session.state === 'queued').length),
      metric(
        'clients',
        'Live clients',
        sessions.reduce((total, session) => total + session.status.connection_counts.total_clients, 0),
      ),
    ],
    rows: sessions.map(sessionRow),
  };
}

export function sessionRow(session: SessionResource): SessionOverviewRow {
  const attention = attentionLabel(session);
  return {
    id: session.id,
    shortId: shortId(session.id),
    state: session.state,
    stateTone: stateTone(session),
    runtime: session.status.runtime_state,
    presence: session.status.presence_state,
    clients: String(session.status.connection_counts.total_clients),
    project: session.project?.name ?? session.project_id ?? 'owner scope',
    admission: session.admission ? `${session.admission.state}: ${session.admission.reason_code}` : 'owner scope',
    template: session.template_id ?? 'none',
    browserContext: browserContextLabel(session),
    egress: egressLabel(session),
    capabilities: capabilityLabel(session),
    attention,
    attentionTone: attention === 'none' ? 'neutral' : session.state === 'queued' ? 'warning' : 'danger',
    updatedAt: formatDateTime(session.updated_at),
  };
}

function metric(key: string, label: string, value: number): SessionOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `sessions-metric-${key}`,
  };
}

function isActiveSession(session: SessionResource): boolean {
  return !['stopped', 'killed', 'cancelled', 'failed'].includes(session.state)
    && session.status.runtime_state !== 'stopped';
}

function shortId(sessionId: string): string {
  return sessionId.length > 13 ? `${sessionId.slice(0, 8)}...${sessionId.slice(-4)}` : sessionId;
}

function stateTone(session: SessionResource): ProjectTone {
  if (session.state === 'queued') {
    return 'warning';
  }
  if (['stopped', 'released', 'cancelled'].includes(session.state)) {
    return 'neutral';
  }
  if (session.state === 'failed' || session.egress_diagnostics?.health === 'blocked') {
    return 'danger';
  }
  return 'success';
}

function browserContextLabel(session: SessionResource): string {
  if (session.browser_context.mode === 'reusable' && session.browser_context.context_id) {
    return `reusable ${session.browser_context.context_id}`;
  }
  return session.browser_context.mode;
}

function egressLabel(session: SessionResource): string {
  if (session.effective_egress?.profile_name) {
    return session.effective_egress.profile_name;
  }
  if (session.effective_egress?.profile_id) {
    return session.effective_egress.profile_id;
  }
  if (session.network_identity?.egress_profile_id) {
    return session.network_identity.egress_profile_id;
  }
  return 'direct';
}

function capabilityLabel(session: SessionResource): string {
  const enabled = Object.entries(session.capabilities)
    .filter(([, value]) => value)
    .map(([key]) => key.replace(/_/g, ' '));
  return enabled.length ? enabled.join(', ') : 'none';
}

function attentionLabel(session: SessionResource): string {
  if (session.state === 'queued' && session.queue) {
    return `queued #${session.queue.position}`;
  }
  if (session.admission?.state === 'rejected') {
    return session.admission.reason_code;
  }
  if (session.egress_diagnostics?.health && !['ready', 'unknown'].includes(session.egress_diagnostics.health)) {
    return `egress ${session.egress_diagnostics.health}`;
  }
  if (!session.status.stop_eligibility.allowed && session.status.stop_eligibility.blockers.length > 0) {
    return session.status.stop_eligibility.blockers
      .map((blocker) => `${blocker.count} ${blocker.kind}`)
      .join(', ');
  }
  return 'none';
}
