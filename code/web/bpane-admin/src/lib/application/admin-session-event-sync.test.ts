import { describe, expect, it } from 'vitest';
import type { AdminEventClient, AdminEventHandlers } from '../api/admin-event-client';
import type { SessionResource } from '../api/control-types';
import type { AdminLogEntry } from '../presentation/logs-view-model';
import { subscribeAdminSessionEvents } from './admin-session-event-sync';

describe('subscribeAdminSessionEvents', () => {
  it('syncs session snapshots and emits typed log entries', () => {
    const client = new FakeAdminEventClient();
    const logs: AdminLogEntry[] = [];
    const errors: Array<string | null> = [];
    const loading: boolean[] = [];
    const sessions: Array<readonly { readonly id: string }[]> = [];

    const subscription = subscribeAdminSessionEvents(client as never, {
      onSessions: (next) => sessions.push(next),
      onLoadingChange: (next) => loading.push(next),
      onError: (next) => errors.push(next),
      onLog: (entry) => logs.push(entry),
    });
    client.handlers.onStatus?.('open');
    client.handlers.onEvent({
      type: 'sessions.snapshot',
      sequence: 3,
      createdAt: '2026-05-04T19:02:00Z',
      sessions: [{ id: 'session-a' }],
    } as never);
    subscription.close();

    expect(loading).toEqual([false]);
    expect(errors).toEqual([null]);
    expect(sessions).toEqual([[{ id: 'session-a' }]]);
    expect(logs.map((entry) => entry.source)).toEqual(['ui', 'gateway']);
  });

  it('deduplicates unchanged snapshot log entries while keeping panels in sync', () => {
    const client = new FakeAdminEventClient();
    const logs: AdminLogEntry[] = [];
    const sessions: Array<readonly SessionResource[]> = [];
    const activeSession = sessionResource({ state: 'active', updated_at: '2026-05-04T19:02:00Z' });

    subscribeAdminSessionEvents(client as never, {
      onSessions: (next) => sessions.push(next),
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: (entry) => logs.push(entry),
    });
    client.handlers.onEvent({
      type: 'sessions.snapshot',
      sequence: 3,
      createdAt: '2026-05-04T19:02:00Z',
      sessions: [activeSession],
    });
    client.handlers.onEvent({
      type: 'sessions.snapshot',
      sequence: 4,
      createdAt: '2026-05-04T19:02:01Z',
      sessions: [sessionResource({ state: 'active', updated_at: '2026-05-04T19:02:01Z' })],
    });
    client.handlers.onEvent({
      type: 'sessions.snapshot',
      sequence: 5,
      createdAt: '2026-05-04T19:02:02Z',
      sessions: [sessionResource({ state: 'idle', updated_at: '2026-05-04T19:02:02Z' })],
    });

    expect(sessions).toHaveLength(3);
    expect(logs.map((entry) => entry.message)).toEqual([
      'Gateway session snapshot #3: 1 visible sessions.',
      'Gateway session snapshot #5: 1 visible sessions.',
    ]);
  });

  it('surfaces stream errors as UI diagnostics', () => {
    const client = new FakeAdminEventClient();
    const logs: AdminLogEntry[] = [];
    const errors: Array<string | null> = [];

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: (next) => errors.push(next),
      onLog: (entry) => logs.push(entry),
    });
    client.handlers.onError?.(new Error('socket failed'));

    expect(errors).toEqual(['socket failed']);
    expect(logs[0]?.source).toBe('ui');
    expect(logs[0]?.message).toBe('Admin event stream error: socket failed');
  });

  it('notifies panel refresh boundaries when session files change', () => {
    const client = new FakeAdminEventClient();
    const snapshots: Array<readonly { readonly sessionId: string; readonly fileCount: number }[]> = [];

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onSessionFilesSnapshot: (next) => snapshots.push(next),
    });
    client.handlers.onEvent({
      type: 'session_files.snapshot',
      sequence: 4,
      createdAt: '2026-05-04T19:02:00Z',
      sessionFiles: [{ sessionId: 'session-a', fileCount: 1, latestUpdatedAt: null }],
    });

    expect(snapshots).toEqual([[{ sessionId: 'session-a', fileCount: 1, latestUpdatedAt: null }]]);
  });

  it('passes workflow run snapshots to the follow handler', () => {
    const client = new FakeAdminEventClient();
    const runs: Array<readonly { readonly id: string; readonly sessionId: string }[]> = [];

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onWorkflowRunsSnapshot: (next) => runs.push(next),
    });
    client.handlers.onEvent({
      type: 'workflow_runs.snapshot',
      sequence: 5,
      createdAt: '2026-05-04T19:02:00Z',
      workflowRuns: [{ id: 'run-a', sessionId: 'session-a', state: 'running', updatedAt: '2026-05-04T19:01:00Z' }],
    });

    expect(runs).toEqual([[{ id: 'run-a', sessionId: 'session-a', state: 'running', updatedAt: '2026-05-04T19:01:00Z' }]]);
  });

  it('notifies panel refresh boundaries when recordings change', () => {
    const client = new FakeAdminEventClient();
    const snapshots: Array<readonly { readonly sessionId: string; readonly readyCount: number }[]> = [];

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onRecordingsSnapshot: (next) => snapshots.push(next),
    });
    client.handlers.onEvent({
      type: 'recordings.snapshot',
      sequence: 5,
      createdAt: '2026-05-04T19:02:00Z',
      recordings: [{
        sessionId: 'session-a',
        recordingCount: 1,
        activeCount: 0,
        readyCount: 1,
        latestUpdatedAt: null,
      }],
    });

    expect(snapshots).toEqual([[
      {
        sessionId: 'session-a',
        recordingCount: 1,
        activeCount: 0,
        readyCount: 1,
        latestUpdatedAt: null,
      },
    ]]);
  });

  it('notifies panel refresh boundaries when MCP delegation changes', () => {
    const client = new FakeAdminEventClient();
    const snapshots: Array<readonly { readonly sessionId: string; readonly delegatedClientId: string | null }[]> = [];

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onMcpDelegationSnapshot: (next) => snapshots.push(next),
    });
    client.handlers.onEvent({
      type: 'mcp_delegation.snapshot',
      sequence: 6,
      createdAt: '2026-05-04T19:02:00Z',
      delegations: [{
        sessionId: 'session-a',
        delegatedClientId: 'bpane-mcp-bridge',
        delegatedIssuer: 'local-compose',
        mcpOwner: false,
        updatedAt: '2026-05-04T19:01:00Z',
      }],
    });

    expect(snapshots).toEqual([[
      {
        sessionId: 'session-a',
        delegatedClientId: 'bpane-mcp-bridge',
        delegatedIssuer: 'local-compose',
        mcpOwner: false,
        updatedAt: '2026-05-04T19:01:00Z',
      },
    ]]);
  });
});

class FakeAdminEventClient implements Pick<AdminEventClient, 'subscribe'> {
  handlers!: AdminEventHandlers;

  subscribe(handlers: AdminEventHandlers) {
    this.handlers = handlers;
    return { close: () => undefined };
  }
}

function sessionResource(overrides: Partial<SessionResource> = {}): SessionResource {
  return {
    id: 'session-a',
    state: 'active',
    owner_mode: 'collaborative',
    idle_timeout_sec: null,
    labels: {},
    connect: {
      gateway_url: 'https://gateway.example',
      transport_path: '/session',
      auth_type: 'session_connect_ticket',
      ticket_path: '/api/v1/sessions/session-a/access-tokens',
      compatibility_mode: 'session_runtime_pool',
    },
    runtime: {
      binding: 'docker_pool',
      compatibility_mode: 'session_runtime_pool',
      cdp_endpoint: null,
    },
    status: {
      runtime_state: 'running',
      runtime_resume_mode: 'exact_live',
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
    created_at: '2026-05-04T19:01:00Z',
    updated_at: '2026-05-04T19:02:00Z',
    runtime_released_at: null,
    stopped_at: null,
    ...overrides,
  };
}
