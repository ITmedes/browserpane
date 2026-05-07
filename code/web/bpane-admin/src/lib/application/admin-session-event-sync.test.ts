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
});

class FakeAdminEventClient implements Pick<AdminEventClient, 'subscribe'> {
  handlers!: AdminEventHandlers;

  subscribe(handlers: AdminEventHandlers) {
    this.handlers = handlers;
    return { close: () => undefined };
  }
}
