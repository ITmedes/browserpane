import { describe, expect, it } from 'vitest';
import type { AdminEventClient, AdminEventHandlers } from '../api/admin-event-client';
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
    let refreshes = 0;

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onSessionFilesSnapshot: () => { refreshes += 1; },
    });
    client.handlers.onEvent({
      type: 'session_files.snapshot',
      sequence: 4,
      createdAt: '2026-05-04T19:02:00Z',
      sessionFiles: [{ sessionId: 'session-a', fileCount: 1, latestUpdatedAt: null }],
    });

    expect(refreshes).toBe(1);
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
    let refreshes = 0;

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onRecordingsSnapshot: () => { refreshes += 1; },
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

    expect(refreshes).toBe(1);
  });

  it('notifies panel refresh boundaries when MCP delegation changes', () => {
    const client = new FakeAdminEventClient();
    let refreshes = 0;

    subscribeAdminSessionEvents(client as never, {
      onSessions: () => undefined,
      onLoadingChange: () => undefined,
      onError: () => undefined,
      onLog: () => undefined,
      onMcpDelegationSnapshot: () => { refreshes += 1; },
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

    expect(refreshes).toBe(1);
  });
});

class FakeAdminEventClient implements Pick<AdminEventClient, 'subscribe'> {
  handlers!: AdminEventHandlers;

  subscribe(handlers: AdminEventHandlers) {
    this.handlers = handlers;
    return { close: () => undefined };
  }
}
