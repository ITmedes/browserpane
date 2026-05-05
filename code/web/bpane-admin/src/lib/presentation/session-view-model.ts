import type { SessionResource } from '../api/control-types';

export type SessionListItemViewModel = {
  readonly id: string;
  readonly lifecycle: string;
  readonly runtime: string;
  readonly presence: string;
  readonly clients: number;
};

export type SessionListPanelViewModel = {
  readonly sessions: readonly SessionListItemViewModel[];
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

export type SessionDetailPanelViewModel = {
  readonly title: string;
  readonly facts: readonly SessionFactViewModel[];
  readonly hint: string | null;
  readonly canRefresh: boolean;
  readonly canStop: boolean;
  readonly canKill: boolean;
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
    return {
      sessions: input.sessions.map((session) => ({
        id: session.id,
        lifecycle: session.state,
        runtime: session.status.runtime_state,
        presence: session.status.presence_state,
        clients: session.status.connection_counts.total_clients,
      })),
      selectedSessionId: input.selectedSessionId,
      authenticated: input.authenticated,
      loading: input.loading,
      error: input.error,
    };
  }

  static detail(input: {
    readonly session: SessionResource | null;
    readonly connected: boolean;
    readonly loading: boolean;
    readonly error: string | null;
  }): SessionDetailPanelViewModel {
    const session = input.session;
    if (!session) {
      return {
        title: 'No session selected',
        facts: [],
        hint: 'Select or create a session to inspect lifecycle and runtime state.',
        canRefresh: false,
        canStop: false,
        canKill: false,
        loading: input.loading,
        error: input.error,
      };
    }
    return {
      title: session.id,
      facts: [
        { label: 'state', value: session.state, testId: 'session-state' },
        { label: 'owner', value: session.owner_mode },
        { label: 'runtime', value: session.status.runtime_state, testId: 'session-runtime-state' },
        { label: 'presence', value: session.status.presence_state, testId: 'session-presence-state' },
        { label: 'binding', value: session.runtime.binding },
        { label: 'transport', value: session.connect.compatibility_mode },
      ],
      hint: resolveHint(session, input.connected),
      canRefresh: !input.loading,
      canStop: !input.loading && !input.connected && session.status.stop_eligibility.allowed,
      canKill: !input.loading && !input.connected,
      loading: input.loading,
      error: input.error,
    };
  }
}

function resolveHint(session: SessionResource, connected: boolean): string | null {
  if (connected) {
    return 'Disconnect the embedded browser before stopping this session.';
  }
  if (session.status.stop_eligibility.allowed) {
    return null;
  }
  const blockers = session.status.stop_eligibility.blockers;
  const reason = blockers.length === 0
    ? 'the current runtime state'
    : blockers.map((blocker) => `${blocker.count} ${blocker.kind}`).join(', ');
  return `Stop is blocked by ${reason}.`;
}
