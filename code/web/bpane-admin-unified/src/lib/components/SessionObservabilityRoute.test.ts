import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload, sessionStatusPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionObservabilityRoute from './SessionObservabilityRoute.svelte';

let sockets: FakeWebSocket[] = [];

afterEach(async () => {
  vi.unstubAllGlobals();
  sockets = [];
  await cleanupRenderedComponents();
});

describe('SessionObservabilityRoute', () => {
  it('authenticates the admin event stream and updates session-scoped evidence live', async () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/admin/events/access-tokens')) {
        return jsonResponse(accessResponse());
      }
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ mcpOwner: false }));
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload());
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionObservabilityRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => expect(sockets).toHaveLength(1));
    expect(sockets[0]?.url).toBe('ws://localhost:3000/api/v1/admin/events');
    sockets[0]?.open();
    expect(sockets[0]?.sent[0]).toContain('event-token');
    sockets[0]?.message({ message_type: 'admin.authenticated' });
    sockets[0]?.message(event('sessions.snapshot', { sessions: [sessionPayload({ totalClients: 2 })] }));
    sockets[0]?.message(event('workflow_runs.snapshot', {
      workflow_runs: [
        { id: 'run-1', session_id: 'session-1', state: 'running', updated_at: '2026-08-07T10:01:00Z' },
        { id: 'run-2', session_id: 'session-2', state: 'failed', updated_at: '2026-08-07T10:01:00Z' },
      ],
    }));
    sockets[0]?.message(event('session_files.snapshot', {
      session_files: [{ session_id: 'session-1', file_count: 3, latest_updated_at: '2026-08-07T10:01:00Z' }],
    }));
    sockets[0]?.message(event('recordings.snapshot', {
      recordings: [{ session_id: 'session-1', recording_count: 2, active_count: 1, ready_count: 1, latest_updated_at: '2026-08-07T10:01:00Z' }],
    }));
    sockets[0]?.message(event('mcp_delegation.snapshot', {
      mcp_delegations: [{
        session_id: 'session-1',
        delegated_client_id: 'bpane-mcp-bridge',
        delegated_issuer: 'local-compose',
        mcp_owner: true,
        updated_at: '2026-08-07T10:01:00Z',
      }],
    }));

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-observability-stream-status').textContent).toContain('Live');
      expect(byTestId(target, 'session-observability-connections').textContent).toContain('2');
      expect(byTestId(target, 'session-observability-workflows').textContent).toContain('1 total / 1 active');
      expect(byTestId(target, 'session-observability-files').textContent).toContain('3');
      expect(byTestId(target, 'session-observability-recordings').textContent).toContain('2 segments / 1 active');
      expect(byTestId(target, 'session-observability-mcp-owner').textContent).toContain('Active');
    });
    expect(byTestId(target, 'session-subarea-observability').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-observability-timeline-list').children).toHaveLength(5);
  });

  it('keeps current state visible while stream access is unavailable', async () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/admin/events/access-tokens')) {
        return new Response('unavailable', { status: 503 });
      }
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload());
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload());
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionObservabilityRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-observability-stream-description').textContent).toContain('HTTP 503');
    });
    expect(byTestId(target, 'session-observability-runtime').textContent).toContain('running');
    expect(sockets).toHaveLength(0);
  });

  it('delegates stream-token authentication failures to the shared shell handler', async () => {
    vi.stubGlobal('WebSocket', FakeWebSocket);
    const onAuthenticationFailure = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/admin/events/access-tokens')) {
        return new Response('expired', { status: 401 });
      }
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload());
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload());
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    renderComponent(SessionObservabilityRoute, {
      authContext: authContext({ onAuthenticationFailure }),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => expect(onAuthenticationFailure).toHaveBeenCalled());
    expect(sockets).toHaveLength(0);
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: { configured: true, authenticated: true, username: 'demo', accessToken: 'token', claims: null },
    authConfig: null,
    accessTokenProvider: async () => 'owner-token',
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function event(event_type: string, body: Record<string, unknown>) {
  return {
    event_type,
    sequence: 1,
    created_at: '2026-08-07T10:01:00Z',
    ...body,
  };
}

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

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

class FakeWebSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: { readonly data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  readonly sent: string[] = [];

  constructor(readonly url: string) {
    sockets.push(this);
  }

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
