import { describe, expect, it, vi } from 'vitest';
import { AdminEventMapper } from './admin-event-mapper';
import {
  AdminEventClient,
  buildAdminEventUrl,
  type AdminEventWebSocket,
} from './admin-event-client';

const SESSION = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'ready',
  owner_mode: 'collaborative',
  connect: {
    gateway_url: 'https://localhost:4433',
    transport_path: '/session',
    auth_type: 'session_connect_ticket',
    ticket_path: '/api/v1/sessions/019df4d2-f4f7-7b00-9e0c-79683b1c82f6/access-tokens',
    compatibility_mode: 'session_runtime_pool',
  },
  runtime: {
    binding: 'docker_runtime_pool',
    compatibility_mode: 'session_runtime_pool',
  },
  status: {
    runtime_state: 'not_started',
    presence_state: 'empty',
    connection_counts: {
      interactive_clients: 0,
      owner_clients: 0,
      viewer_clients: 0,
      recorder_clients: 0,
      automation_clients: 0,
      total_clients: 0,
    },
    stop_eligibility: {
      allowed: true,
      blockers: [],
    },
  },
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
  stopped_at: null,
};

describe('AdminEventMapper', () => {
  it('maps session snapshot events through the control session mapper', () => {
    const event = AdminEventMapper.toEvent({
      event_type: 'sessions.snapshot',
      sequence: 2,
      created_at: '2026-05-04T19:02:00Z',
      sessions: [SESSION],
    });

    expect(event.type).toBe('sessions.snapshot');
    if (event.type === 'sessions.snapshot') {
      expect(event.sessions[0]?.id).toBe(SESSION.id);
      expect(event.sessions[0]?.status.runtime_state).toBe('not_started');
    }
  });

  it('maps workflow run snapshot events', () => {
    const event = AdminEventMapper.toEvent({
      event_type: 'workflow_runs.snapshot',
      sequence: 5,
      created_at: '2026-05-04T19:02:00Z',
      workflow_runs: [{
        id: 'run-a',
        session_id: SESSION.id,
        state: 'running',
        updated_at: '2026-05-04T19:01:00Z',
      }],
    });

    expect(event.type).toBe('workflow_runs.snapshot');
    if (event.type === 'workflow_runs.snapshot') {
      expect(event.workflowRuns[0]?.id).toBe('run-a');
      expect(event.workflowRuns[0]?.state).toBe('running');
    }
  });

  it('maps session file snapshot events', () => {
    const event = AdminEventMapper.toEvent({
      event_type: 'session_files.snapshot',
      sequence: 6,
      created_at: '2026-05-04T19:02:00Z',
      session_files: [{
        session_id: SESSION.id,
        file_count: 2,
        latest_updated_at: '2026-05-04T19:01:00Z',
      }],
    });

    expect(event.type).toBe('session_files.snapshot');
    if (event.type === 'session_files.snapshot') {
      expect(event.sessionFiles[0]?.sessionId).toBe(SESSION.id);
      expect(event.sessionFiles[0]?.fileCount).toBe(2);
    }
  });

  it('maps recording snapshot events', () => {
    const event = AdminEventMapper.toEvent({
      event_type: 'recordings.snapshot',
      sequence: 7,
      created_at: '2026-05-04T19:02:00Z',
      recordings: [{
        session_id: SESSION.id,
        recording_count: 2,
        active_count: 1,
        ready_count: 1,
        latest_updated_at: '2026-05-04T19:01:00Z',
      }],
    });

    expect(event.type).toBe('recordings.snapshot');
    if (event.type === 'recordings.snapshot') {
      expect(event.recordings[0]?.sessionId).toBe(SESSION.id);
      expect(event.recordings[0]?.activeCount).toBe(1);
    }
  });

  it('rejects unsupported event types at the API boundary', () => {
    expect(() =>
      AdminEventMapper.toEvent({
        event_type: 'sessions.delta',
        sequence: 1,
        created_at: '2026-05-04T19:02:00Z',
      }),
    ).toThrow('unsupported admin event type: sessions.delta');
  });
});

describe('AdminEventClient', () => {
  it('builds browser-compatible websocket urls from http origins', () => {
    expect(buildAdminEventUrl('http://localhost:8080/admin/', 'token value')).toBe(
      'ws://localhost:8080/api/v1/admin/events?access_token=token+value',
    );
    expect(buildAdminEventUrl('https://browserpane.example/admin/', 'secure-token')).toBe(
      'wss://browserpane.example/api/v1/admin/events?access_token=secure-token',
    );
  });

  it('opens a websocket and maps incoming events', async () => {
    const events: string[] = [];
    const received: string[] = [];
    const sockets: FakeAdminEventWebSocket[] = [];
    const client = new AdminEventClient({
      baseUrl: 'http://localhost:8080/admin/',
      accessTokenProvider: () => 'owner-token',
      webSocketFactory: (url) => {
        sockets.push(new FakeAdminEventWebSocket(url));
        return sockets.at(-1)!;
      },
    });

    const subscription = client.subscribe({
      onEvent: (event) => received.push(event.type),
      onStatus: (status) => events.push(status),
    });
    await flushMicrotasks();
    sockets[0]?.open();
    sockets[0]?.message({
      event_type: 'sessions.snapshot',
      sequence: 1,
      created_at: '2026-05-04T19:02:00Z',
      sessions: [SESSION],
    });
    subscription.close();

    expect(sockets[0]?.url).toBe(
      'ws://localhost:8080/api/v1/admin/events?access_token=owner-token',
    );
    expect(events).toEqual(['connecting', 'open', 'closed']);
    expect(received).toEqual(['sessions.snapshot']);
  });

  it('reconnects with a fresh access token after server close', async () => {
    vi.useFakeTimers();
    const sockets: FakeAdminEventWebSocket[] = [];
    const tokens = ['first-token', 'second-token'];
    const client = new AdminEventClient({
      baseUrl: 'https://browserpane.example/admin/',
      accessTokenProvider: () => tokens.shift() ?? 'fallback-token',
      reconnectDelayMs: 25,
      webSocketFactory: (url) => {
        sockets.push(new FakeAdminEventWebSocket(url));
        return sockets.at(-1)!;
      },
    });
    const subscription = client.subscribe({ onEvent: () => undefined });
    await flushMicrotasks();

    sockets[0]?.serverClose();
    await vi.advanceTimersByTimeAsync(25);

    expect(sockets).toHaveLength(2);
    expect(sockets[1]?.url).toContain('access_token=second-token');
    subscription.close();
    vi.useRealTimers();
  });
});

class FakeAdminEventWebSocket implements AdminEventWebSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: { readonly data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;

  constructor(readonly url: string) {}

  close(): void {
    this.onclose?.();
  }

  open(): void {
    this.onopen?.();
  }

  serverClose(): void {
    this.onclose?.();
  }

  message(payload: unknown): void {
    this.onmessage?.({ data: JSON.stringify(payload) });
  }
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
