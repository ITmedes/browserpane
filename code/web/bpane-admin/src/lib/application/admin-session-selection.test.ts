import { describe, expect, it } from 'vitest';
import type { SessionResource } from '../api/control-types';
import { AdminSessionSelection } from './admin-session-selection';

const OLD_SESSION = session('old-session');
const NEW_SESSION = session('new-session');

describe('AdminSessionSelection', () => {
  it('keeps an optimistically created session selected until it appears in the list', () => {
    const selection = AdminSessionSelection.afterList({
      sessions: [OLD_SESSION],
      selectedSession: NEW_SESSION,
      pendingSelectedSessionId: NEW_SESSION.id,
    });

    expect(selection.selectedSession?.id).toBe(NEW_SESSION.id);
    expect(selection.pendingSelectedSessionId).toBe(NEW_SESSION.id);
  });

  it('clears pending selection once the refreshed list contains the created session', () => {
    const selection = AdminSessionSelection.afterList({
      sessions: [OLD_SESSION, NEW_SESSION],
      selectedSession: NEW_SESSION,
      pendingSelectedSessionId: NEW_SESSION.id,
    });

    expect(selection.selectedSession?.id).toBe(NEW_SESSION.id);
    expect(selection.pendingSelectedSessionId).toBeNull();
  });
});

function session(id: string): SessionResource {
  return {
    id,
    state: 'active',
    owner_mode: 'shared',
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: '/session',
      auth_type: 'session_connect_ticket',
      compatibility_mode: 'session_runtime_pool',
    },
    runtime: {
      binding: 'docker_runtime_pool',
      compatibility_mode: 'session_runtime_pool',
    },
    status: {
      runtime_state: 'running',
      presence_state: 'empty',
      connection_counts: {
        interactive_clients: 0,
        owner_clients: 0,
        viewer_clients: 0,
        recorder_clients: 0,
        automation_clients: 0,
        total_clients: 0,
      },
      stop_eligibility: { allowed: true, blockers: [] },
    },
    created_at: '2026-05-04T19:00:00Z',
    updated_at: '2026-05-04T19:01:00Z',
    stopped_at: null,
  };
}
