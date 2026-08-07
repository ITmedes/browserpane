import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  AdminEventStreamClient,
  type AdminEventWebSocket,
} from './admin-event-stream-client';

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('AdminEventStreamClient', () => {
  it('authenticates over the socket before mapping events', async () => {
    const sockets: FakeSocket[] = [];
    const statuses: string[] = [];
    const events: string[] = [];
    const client = streamClient(sockets, {
      mapEvent: (payload) => String((payload as Record<string, unknown>).event_type),
    });

    const subscription = client.subscribe({
      onEvent: (event) => events.push(event),
      onStatus: (status) => statuses.push(status),
    });
    await vi.waitFor(() => expect(sockets).toHaveLength(1));
    sockets[0]?.open();
    sockets[0]?.message({ message_type: 'admin.authenticated' });
    sockets[0]?.message({ event_type: 'sessions.snapshot' });
    subscription.close();

    expect(sockets[0]?.url).toBe('wss://pane.example/api/v1/admin/events');
    expect(sockets[0]?.sent).toEqual([
      JSON.stringify({ message_type: 'admin.authenticate', token: 'event-token' }),
    ]);
    expect(statuses).toEqual(['connecting', 'open', 'closed']);
    expect(events).toEqual(['sessions.snapshot']);
  });

  it('reconnects with newly issued access after a server close', async () => {
    vi.useFakeTimers();
    const sockets: FakeSocket[] = [];
    const tokens = ['first-token', 'second-token'];
    const client = streamClient(sockets, {
      issueAccessToken: async () => access(tokens.shift() ?? 'fallback'),
      reconnectDelayMs: 25,
    });
    const subscription = client.subscribe({ onEvent: () => undefined });
    await vi.advanceTimersByTimeAsync(0);
    sockets[0]?.open();
    sockets[0]?.message({ message_type: 'admin.authenticated' });
    sockets[0]?.serverClose();
    await vi.advanceTimersByTimeAsync(25);
    sockets[1]?.open();

    expect(sockets).toHaveLength(2);
    expect(sockets[0]?.sent[0]).toContain('first-token');
    expect(sockets[1]?.sent[0]).toContain('second-token');
    subscription.close();
  });

  it('reports handshake and event mapping errors and probes authentication', async () => {
    const sockets: FakeSocket[] = [];
    const errors: string[] = [];
    const probe = vi.fn(async () => undefined);
    const client = streamClient(sockets, {
      probeAuthentication: probe,
      mapEvent: () => {
        throw new Error('invalid event');
      },
    });
    const subscription = client.subscribe({
      onEvent: () => undefined,
      onError: (error) => errors.push(error.message),
    });
    await vi.waitFor(() => expect(sockets).toHaveLength(1));
    sockets[0]?.open();
    sockets[0]?.message({ message_type: 'wrong' });
    await vi.waitFor(() => expect(probe).toHaveBeenCalled());
    expect(errors).toContain('admin event stream rejected its authentication handshake');
    subscription.close();

    const eventSockets: FakeSocket[] = [];
    const eventClient = streamClient(eventSockets, {
      probeAuthentication: probe,
      mapEvent: () => {
        throw new Error('invalid event');
      },
    });
    const eventSubscription = eventClient.subscribe({
      onEvent: () => undefined,
      onError: (error) => errors.push(error.message),
    });
    await vi.waitFor(() => expect(eventSockets).toHaveLength(1));
    eventSockets[0]?.open();
    eventSockets[0]?.message({ message_type: 'admin.authenticated' });
    eventSockets[0]?.message({ event_type: 'bad' });
    eventSockets[0]?.networkError();
    await vi.waitFor(() => expect(probe).toHaveBeenCalledTimes(2));
    expect(errors).toContain('invalid event');
    expect(errors).toContain('admin event stream websocket error');
    eventSubscription.close();
  });

  it('reports access issuance failures without opening a socket', async () => {
    const sockets: FakeSocket[] = [];
    const client = streamClient(sockets, {
      issueAccessToken: async () => {
        throw new Error('token unavailable');
      },
    });
    const error = new Promise<Error>((resolve) => {
      const subscription = client.subscribe({
        onEvent: () => undefined,
        onError: resolve,
      });
      setTimeout(() => subscription.close(), 10);
    });

    expect((await error).message).toBe('token unavailable');
    expect(sockets).toHaveLength(0);
  });

  it('uses the browser WebSocket constructor when no factory is provided', async () => {
    const sockets: FakeSocket[] = [];
    class BrowserWebSocket extends FakeSocket {
      constructor(url: string) {
        super(url);
        sockets.push(this);
      }
    }
    vi.stubGlobal('WebSocket', BrowserWebSocket);
    const client = new AdminEventStreamClient({
      baseUrl: 'http://pane.example/admin-new/',
      issueAccessToken: async () => access('browser-token'),
      mapEvent: (payload) => payload,
    });

    const subscription = client.subscribe({ onEvent: () => undefined });
    await vi.waitFor(() => expect(sockets).toHaveLength(1));
    sockets[0]?.open();

    expect(sockets[0]?.url).toBe('ws://pane.example/api/v1/admin/events');
    expect(sockets[0]?.sent[0]).toContain('browser-token');
    subscription.close();
  });

  it('absorbs authentication-probe failures after a socket error', async () => {
    const sockets: FakeSocket[] = [];
    const probe = vi.fn(async () => {
      throw new Error('authentication unavailable');
    });
    const client = streamClient(sockets, { probeAuthentication: probe });
    const subscription = client.subscribe({ onEvent: () => undefined });
    await vi.waitFor(() => expect(sockets).toHaveLength(1));

    sockets[0]?.networkError();
    await vi.waitFor(() => expect(probe).toHaveBeenCalled());
    subscription.close();
  });
});

function streamClient<Event = unknown>(
  sockets: FakeSocket[],
  overrides: Partial<ConstructorParameters<typeof AdminEventStreamClient<Event>>[0]> = {},
): AdminEventStreamClient<Event> {
  return new AdminEventStreamClient<Event>({
    baseUrl: 'https://pane.example/admin-new/',
    issueAccessToken: async () => access('event-token'),
    mapEvent: (payload) => payload as Event,
    webSocketFactory: (url) => {
      const socket = new FakeSocket(url);
      sockets.push(socket);
      return socket;
    },
    reconnectDelayMs: 1_000,
    ...overrides,
  });
}

function access(token: string) {
  return {
    token,
    endpointPath: '/api/v1/admin/events',
    authenticationMessageType: 'admin.authenticate',
    authenticatedMessageType: 'admin.authenticated',
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

  serverClose(): void {
    this.onclose?.();
  }

  networkError(): void {
    this.onerror?.();
  }

  message(payload: unknown): void {
    this.onmessage?.({ data: JSON.stringify(payload) });
  }
}
