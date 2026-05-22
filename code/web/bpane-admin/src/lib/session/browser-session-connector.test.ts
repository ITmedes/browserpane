import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from '../api/control-client';
import type { SessionResource } from '../api/control-types';
import { BrowserSessionConnector } from './browser-session-connector';
import {
  DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
  type BrowserSessionConnectOptions,
} from './browser-session-types';

const SESSION: SessionResource = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  browser_context: { mode: 'fresh', context_id: null },
  owner_mode: 'shared',
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
    cdp_endpoint: 'http://runtime:9223',
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
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
  stopped_at: null,
};

describe('BrowserSessionConnector', () => {
  it('mints a connect ticket before connecting through the SDK', async () => {
    let connectOptions: BrowserSessionConnectOptions | null = null;
    const handle = { disconnect: vi.fn() };
    const connector = new BrowserSessionConnector({
      controlClient: new ControlClient({
        baseUrl: 'http://localhost:8932',
        accessTokenProvider: () => 'owner-token',
        fetchImpl: ticketFetch(),
      }),
      sdkProvider: {
        load: async () => ({
          BpaneSession: {
            connect: async (options) => {
              connectOptions = options;
              return handle;
            },
          },
        }),
      },
    });
    const container = document.createElement('div');

    const connection = await connector.connect(SESSION, container);

    expect(connection.sessionId).toBe(SESSION.id);
    expect(connection.gatewayUrl).toBe('https://localhost:4433/session');
    expect(connectOptions).toMatchObject({
      container,
      gatewayUrl: 'https://localhost:4433/session',
      connectTicket: 'connect-ticket',
      clientRole: 'interactive',
      certHashUrl: '/cert-hash',
      hiDpi: true,
      microphone: true,
      camera: true,
      renderBackend: 'auto',
      scrollCopy: true,
      fileTransfer: true,
    });
  });

  it('forwards admin display connection preferences to the SDK', async () => {
    let connectOptions: BrowserSessionConnectOptions | null = null;
    const connector = new BrowserSessionConnector({
      controlClient: new ControlClient({
        baseUrl: 'http://localhost:8932',
        accessTokenProvider: () => 'owner-token',
        fetchImpl: ticketFetch(),
      }),
      sdkProvider: {
        load: async () => ({
          BpaneSession: {
            connect: async (options) => {
              connectOptions = options;
              return { disconnect: vi.fn() };
            },
          },
        }),
      },
    });

    await connector.connect(SESSION, document.createElement('div'), {
      ...DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES,
      hiDpi: false,
      renderBackend: 'canvas2d',
      scrollCopy: false,
    });

    expect(connectOptions).toMatchObject({
      hiDpi: false,
      renderBackend: 'canvas2d',
      scrollCopy: false,
      microphone: true,
      camera: true,
    });
  });

  it('clears stale SDK mount state before connecting', async () => {
    const handle = { disconnect: vi.fn() };
    const connector = new BrowserSessionConnector({
      controlClient: new ControlClient({
        baseUrl: 'http://localhost:8932',
        accessTokenProvider: () => 'owner-token',
        fetchImpl: ticketFetch(),
      }),
      sdkProvider: {
        load: async () => ({
          BpaneSession: {
            connect: async (options) => {
              expect(options.container.childElementCount).toBe(0);
              expect(options.container.getAttribute('style')).toBeNull();
              return handle;
            },
          },
        }),
      },
    });
    const container = document.createElement('div');
    container.style.width = '1280px';
    container.innerHTML = '<canvas></canvas><canvas></canvas>';

    const connection = await connector.connect(SESSION, container);

    expect(connection.handle).toBe(handle);
  });

  it('adds local compose guidance when WebTransport opening handshake fails', async () => {
    const handshakeError = new Error('Opening handshake failed.');
    const connector = new BrowserSessionConnector({
      controlClient: new ControlClient({
        baseUrl: 'http://localhost:8932',
        accessTokenProvider: () => 'owner-token',
        fetchImpl: ticketFetch(),
      }),
      sdkProvider: {
        load: async () => ({
          BpaneSession: {
            connect: async () => {
              throw handshakeError;
            },
          },
        }),
      },
    });

    let thrown: unknown;
    try {
      await connector.connect(SESSION, document.createElement('div'));
    } catch (error) {
      thrown = error;
    }

    expect(thrown).toBeInstanceOf(Error);
    expect((thrown as Error).message).toContain('WebTransport opening handshake failed');
    expect((thrown as Error).message).toContain('https://localhost:4433/session');
    expect((thrown as Error).message).toContain('--origin-to-force-quic-on=localhost:4433');
    expect((thrown as Error & { cause?: unknown }).cause).toBe(handshakeError);
  });
});

function ticketFetch(): FetchLike {
  return vi.fn<FetchLike>(async () => {
    return new Response(JSON.stringify({
      session_id: SESSION.id,
      token_type: 'session_connect_ticket',
      token: 'connect-ticket',
      expires_at: '2026-05-04T19:05:00Z',
      connect: SESSION.connect,
    }));
  });
}
