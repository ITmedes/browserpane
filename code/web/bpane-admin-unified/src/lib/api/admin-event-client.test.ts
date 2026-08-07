import { describe, expect, it, vi } from 'vitest';

import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  AdminEventClient,
  type AdminEventWebSocket,
} from './admin-event-client';

describe('AdminEventClient', () => {
  it('mints scoped access, authenticates, and maps admin-new session resources', async () => {
    const sockets: FakeSocket[] = [];
    const events: string[] = [];
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      expect(String(input)).toBe('https://pane.example/api/v1/admin/events/access-tokens');
      expect(new Headers(init?.headers).get('authorization')).toBe('Bearer owner-token');
      return Response.json(accessResponse());
    });
    const client = new AdminEventClient({
      baseUrl: 'https://pane.example/admin-new/',
      accessTokenProvider: async () => 'owner-token',
      fetchImpl,
      webSocketFactory: (url) => {
        const socket = new FakeSocket(url);
        sockets.push(socket);
        return socket;
      },
    });

    const subscription = client.subscribe({
      onEvent: (event) => {
        events.push(event.type);
        if (event.type === 'sessions.snapshot') {
          expect(event.sessions[0]?.id).toBe('session-1');
        }
      },
    });
    await vi.waitFor(() => expect(sockets).toHaveLength(1));
    sockets[0]?.open();
    sockets[0]?.message({ message_type: 'admin.authenticated' });
    sockets[0]?.message({
      event_type: 'sessions.snapshot',
      sequence: 1,
      created_at: '2026-08-07T10:00:00Z',
      sessions: [sessionPayload()],
    });
    subscription.close();

    expect(events).toEqual(['sessions.snapshot']);
    expect(sockets[0]?.sent[0]).toContain('event-token');
  });

  it('routes token issuance authentication failures through the shell handler', async () => {
    const onAuthenticationFailure = vi.fn();
    const error = new Promise<Error>((resolve) => {
      const client = new AdminEventClient({
        baseUrl: 'https://pane.example/',
        accessTokenProvider: async () => 'expired-token',
        fetchImpl: async () => new Response('expired', { status: 401 }),
        onAuthenticationFailure,
        webSocketFactory: () => new FakeSocket('unused'),
      });
      const subscription = client.subscribe({ onEvent: () => undefined, onError: resolve });
      setTimeout(() => subscription.close(), 10);
    });

    expect((await error).message).toContain('HTTP 401');
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function accessResponse() {
  return {
    token_type: 'admin_event_access_token',
    token: 'event-token',
    expires_at: '2026-08-07T10:05:00Z',
    websocket: {
      endpoint_path: '/api/v1/admin/events',
      auth_type: 'initial_message',
      authentication_message_type: 'admin.authenticate',
      authenticated_message_type: 'admin.authenticated',
    },
  };
}

class FakeSocket implements AdminEventWebSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: { readonly data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  readonly sent: string[] = [];

  constructor(readonly url: string) {}

  close(): void {
    this.onclose?.();
  }

  send(data: string): void {
    this.sent.push(data);
  }

  open(): void {
    this.onopen?.();
  }

  message(payload: unknown): void {
    this.onmessage?.({ data: JSON.stringify(payload) });
  }
}
