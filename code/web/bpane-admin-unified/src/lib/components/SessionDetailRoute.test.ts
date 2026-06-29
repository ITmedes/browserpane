import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload, sessionStatusPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionDetailRoute from './SessionDetailRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions/session-1');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionDetailRoute', () => {
  it('loads session detail and disconnects clients through the authenticated API', async () => {
    let clientsConnected = true;
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: clientsConnected ? 1 : 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/connections/disconnect-all') && init?.method === 'POST') {
        clientsConnected = false;
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: clientsConnected ? 1 : 0 }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-title').textContent).toContain('session-1');
    });
    expect(byTestId(target, 'session-detail-total-clients').textContent).toContain('1');
    byTestId(target, 'session-disconnect-all').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-action-success').textContent).toContain('disconnected');
    });
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-total-clients').textContent).toContain('0');
    });
    const disconnectCall = fetchImpl.mock.calls.find((call) => String(call[0]).endsWith('/connections/disconnect-all'));
    const headers = disconnectCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-error').textContent).toContain('Session detail unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });
});

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: overrides.accessTokenProvider ?? (async () => 'token'),
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
