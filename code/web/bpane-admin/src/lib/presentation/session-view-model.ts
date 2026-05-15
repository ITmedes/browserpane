import type { SessionResource, SessionStopEligibility } from '../api/control-types';
import type { SessionStatus } from '../api/session-status-types';

export type SessionListItemViewModel = {
  readonly id: string;
  readonly shortId: string;
  readonly lifecycle: string;
  readonly runtime: string;
  readonly presence: string;
  readonly clients: number;
  readonly updatedAt: string;
  readonly mcpDelegation: string;
  readonly labels: string;
};

export type SelectedSessionViewModel = SessionListItemViewModel & {
  readonly ownerMode: string;
  readonly runtimeBinding: string;
};

export type SessionListPanelViewModel = {
  readonly sessions: readonly SessionListItemViewModel[];
  readonly selectedSession: SelectedSessionViewModel | null;
  readonly selectedSessionId: string | null;
  readonly authenticated: boolean;
  readonly loading: boolean;
  readonly error: string | null;
};

export type SessionFactViewModel = {
  readonly label: string;
  readonly value: string;
  readonly testId?: string;
};

export type SessionConnectionViewModel = {
  readonly id: number;
  readonly label: string;
  readonly role: string;
  readonly canDisconnect: boolean;
};

export type SessionDetailPanelViewModel = {
  readonly title: string;
  readonly facts: readonly SessionFactViewModel[];
  readonly connections: readonly SessionConnectionViewModel[];
  readonly hint: string | null;
  readonly statusHint: string | null;
  readonly canRefresh: boolean;
  readonly canStop: boolean;
  readonly canKill: boolean;
  readonly canDisconnectAll: boolean;
  readonly loading: boolean;
  readonly error: string | null;
};

export class SessionViewModelBuilder {
  static list(input: {
    readonly sessions: readonly SessionResource[];
    readonly selectedSessionId: string | null;
    readonly authenticated: boolean;
    readonly loading: boolean;
    readonly error: string | null;
  }): SessionListPanelViewModel {
    const selectedSession = input.sessions.find((session) => session.id === input.selectedSessionId) ?? null;
    return {
      sessions: input.sessions.map(toListItem),
      selectedSession: selectedSession ? toSelectedSession(selectedSession) : null,
      selectedSessionId: input.selectedSessionId,
      authenticated: input.authenticated,
      loading: input.loading,
      error: input.error,
    };
  }

  static detail(input: {
    readonly session: SessionResource | null;
    readonly status?: SessionStatus | null;
    readonly connected: boolean;
    readonly loading: boolean;
    readonly error: string | null;
  }): SessionDetailPanelViewModel {
    const session = input.session;
    if (!session) {
      return {
        title: 'No session selected',
        facts: [],
        connections: [],
        hint: 'Select or create a session to inspect lifecycle and runtime state.',
        statusHint: null,
        canRefresh: false,
        canStop: false,
        canKill: false,
        canDisconnectAll: false,
        loading: input.loading,
        error: input.error,
      };
    }
    const status = input.status ?? null;
    const stopEligibility = status?.stop_eligibility ?? session.status.stop_eligibility;
    const connectionCount = status?.connection_counts.total_clients
      ?? session.status.connection_counts.total_clients;
    return {
      title: session.id,
      facts: [
        { label: 'state', value: session.state, testId: 'session-state' },
        { label: 'owner', value: session.owner_mode, testId: 'session-owner-mode' },
        { label: 'idle override', value: session.idle_timeout_sec?.toString() ?? 'default', testId: 'session-idle-timeout' },
        { label: 'labels', value: labelSummary(session.labels ?? {}), testId: 'session-labels' },
        { label: 'runtime', value: session.status.runtime_state, testId: 'session-runtime-state' },
        { label: 'presence', value: session.status.presence_state, testId: 'session-presence-state' },
        { label: 'clients', value: String(connectionCount), testId: 'session-total-clients' },
        { label: 'binding', value: session.runtime.binding },
        { label: 'transport', value: session.connect.compatibility_mode },
        { label: 'created', value: session.created_at },
        { label: 'updated', value: session.updated_at },
        ...(session.stopped_at ? [{ label: 'stopped', value: session.stopped_at }] : []),
        ...statusFacts(status),
      ],
      connections: status?.connections.map((connection) => ({
        id: connection.connection_id,
        label: `#${connection.connection_id}`,
        role: connection.role,
        canDisconnect: !input.loading,
      })) ?? [],
      hint: resolveHint(input.connected, stopEligibility),
      statusHint: status ? null : 'Live status is loaded from the session status API.',
      canRefresh: !input.loading,
      canStop: !input.loading && !input.connected && stopEligibility.allowed,
      canKill: !input.loading && !input.connected,
      canDisconnectAll: !input.loading && (status?.connections.length ?? 0) > 0,
      loading: input.loading,
      error: input.error,
    };
  }
}

function toListItem(session: SessionResource): SessionListItemViewModel {
  return {
    id: session.id,
    shortId: shortId(session.id),
    lifecycle: session.state,
    runtime: session.status.runtime_state,
    presence: session.status.presence_state,
    clients: session.status.connection_counts.total_clients,
    updatedAt: session.updated_at,
    mcpDelegation: mcpDelegationLabel(session),
    labels: labelSummary(session.labels ?? {}),
  };
}

function toSelectedSession(session: SessionResource): SelectedSessionViewModel {
  return {
    ...toListItem(session),
    ownerMode: session.owner_mode,
    runtimeBinding: session.runtime.binding,
  };
}

function statusFacts(status: SessionStatus | null): SessionFactViewModel[] {
  if (!status) {
    return [];
  }
  return [
    { label: 'status state', value: status.state },
    { label: 'resolution', value: `${status.resolution[0]}x${status.resolution[1]}` },
    { label: 'mcp owner', value: yesNo(status.mcp_owner), testId: 'session-mcp-owner' },
    { label: 'exclusive owner', value: yesNo(status.exclusive_browser_owner) },
    { label: 'idle timeout', value: status.idle.idle_timeout_sec?.toString() ?? 'none' },
    { label: 'recording', value: status.recording.state, testId: 'session-recording-state' },
    { label: 'playback', value: `${status.playback.included_segment_count}/${status.playback.segment_count}` },
    { label: 'join latency avg', value: `${status.telemetry.average_join_latency_ms.toFixed(1)} ms` },
  ];
}

function resolveHint(connected: boolean, stopEligibility: SessionStopEligibility): string | null {
  if (connected) {
    return 'Disconnect the embedded browser before stopping this session.';
  }
  if (stopEligibility.allowed) {
    return null;
  }
  const blockers = stopEligibility.blockers;
  const reason = blockers.length === 0
    ? 'the current runtime state'
    : blockers.map((blocker) => `${blocker.count} ${blocker.kind}`).join(', ');
  return `Stop is blocked by ${reason}.`;
}

function yesNo(value: boolean): string {
  return value ? 'yes' : 'no';
}

function mcpDelegationLabel(session: SessionResource): string {
  return session.automation_delegate ? 'MCP delegated' : 'MCP not delegated';
}

function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
