import { describe, expect, it } from 'vitest';
import type { SessionResource } from '../api/control-types';
import { AdminWorkflowFollowPolicy, AdminWorkflowSessionFollower } from './admin-workflow-follow';

describe('AdminWorkflowFollowPolicy', () => {
  it('selects the latest active workflow run', () => {
    const selected = AdminWorkflowFollowPolicy.selectRun([
      { id: 'old', sessionId: 'session-a', state: 'running', updatedAt: '2026-05-04T19:00:00Z' },
      { id: 'done', sessionId: 'session-b', state: 'succeeded', updatedAt: '2026-05-04T19:02:00Z' },
      { id: 'new', sessionId: 'session-c', state: 'awaiting_input', updatedAt: '2026-05-04T19:03:00Z' },
    ]);

    expect(selected?.id).toBe('new');
    expect(AdminWorkflowFollowPolicy.signature(selected!)).toContain('session-c');
  });

  it('ignores inactive workflow runs', () => {
    expect(AdminWorkflowFollowPolicy.selectRun([
      { id: 'done', sessionId: 'session-b', state: 'succeeded', updatedAt: '2026-05-04T19:02:00Z' },
    ])).toBeNull();
  });

  it('follows active workflow runs by selecting and connecting the run session', async () => {
    const session = createSession('session-c');
    const upserted: SessionResource[] = [];
    let connectRequests = 0;
    const follower = new AdminWorkflowSessionFollower({
      controlClient: { getSession: async () => { throw new Error('unexpected session fetch'); } },
      getSessions: () => [session],
      getConnectedSessionId: () => null,
      upsertSession: (next) => upserted.push(next),
      requestBrowserConnect: () => { connectRequests += 1; },
      onError: () => undefined,
    });

    await follower.followRuns([
      { id: 'run-c', sessionId: 'session-c', state: 'running', updatedAt: '2026-05-04T19:03:00Z' },
    ]);
    await follower.followRuns([
      { id: 'run-c', sessionId: 'session-c', state: 'running', updatedAt: '2026-05-04T19:03:00Z' },
    ]);

    expect(upserted).toEqual([session]);
    expect(connectRequests).toBe(1);
  });

  it('loads the run session when the event arrives before the session list catches up', async () => {
    const session = createSession('session-new');
    const upserted: SessionResource[] = [];
    const follower = new AdminWorkflowSessionFollower({
      controlClient: { getSession: async () => session },
      getSessions: () => [],
      getConnectedSessionId: () => null,
      upsertSession: (next) => upserted.push(next),
      requestBrowserConnect: () => undefined,
      onError: () => undefined,
    });

    await follower.followRuns([
      { id: 'run-new', sessionId: 'session-new', state: 'starting', updatedAt: '2026-05-04T19:03:00Z' },
    ]);

    expect(upserted).toEqual([session]);
  });
});

function createSession(id: string): SessionResource {
  return {
    id,
    state: 'active',
    owner_mode: 'collaborative',
    connect: { gateway_url: 'https://gateway.example', transport_path: '/transport', auth_type: 'ticket', compatibility_mode: 'direct' },
    runtime: { binding: 'docker', compatibility_mode: 'direct' },
    status: {
      runtime_state: 'running',
      presence_state: 'connected',
      connection_counts: {
        interactive_clients: 0, owner_clients: 0, viewer_clients: 0,
        recorder_clients: 0, automation_clients: 0, total_clients: 0,
      },
      stop_eligibility: { allowed: true, blockers: [] },
    },
    created_at: '2026-05-04T19:00:00Z',
    updated_at: '2026-05-04T19:01:00Z',
  };
}
