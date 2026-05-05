import { describe, expect, it } from 'vitest';
import type { SessionResource } from '../api/control-types';
import { SessionViewModelBuilder } from './session-view-model';

const SESSION: SessionResource = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
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
    presence_state: 'connected',
    connection_counts: {
      interactive_clients: 1,
      owner_clients: 1,
      viewer_clients: 0,
      recorder_clients: 0,
      automation_clients: 0,
      total_clients: 1,
    },
    stop_eligibility: {
      allowed: false,
      blockers: [{ kind: 'owner_clients', count: 1 }],
    },
  },
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

describe('SessionViewModelBuilder', () => {
  it('maps sessions to compact list rows', () => {
    const viewModel = SessionViewModelBuilder.list({
      sessions: [SESSION],
      selectedSessionId: SESSION.id,
      authenticated: true,
      loading: false,
      error: null,
    });

    expect(viewModel.sessions[0]).toMatchObject({
      id: SESSION.id,
      lifecycle: 'active',
      runtime: 'running',
      presence: 'connected',
      clients: 1,
    });
  });

  it('disables destructive lifecycle actions while connected', () => {
    const viewModel = SessionViewModelBuilder.detail({
      session: SESSION,
      connected: true,
      loading: false,
      error: null,
    });

    expect(viewModel.canStop).toBe(false);
    expect(viewModel.canKill).toBe(false);
    expect(viewModel.hint).toContain('Disconnect');
  });
});
