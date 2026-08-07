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
    expect(byTestId(target, 'session-subarea-overview').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-subarea-live').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/live');
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

  it('opens the selected session in a popup preview window', async () => {
    const focus = vi.fn();
    const open = vi.spyOn(window, 'open').mockReturnValue({ focus } as unknown as Window);
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }), 200);
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
    byTestId(target, 'session-connect-preview').click();

    expect(open).toHaveBeenCalledWith(
      '/admin-new/sessions/session-1/preview',
      'bpane-session-preview-session-1',
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    expect(focus).toHaveBeenCalledOnce();
  });

  it('updates recording policy for the selected session', async () => {
    let recordingMode = 'disabled';
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1/recording-policy') && init?.method === 'PUT') {
        recordingMode = JSON.parse(String(init.body)).mode;
        return jsonResponse(sessionPayload({ totalClients: 0, recordingMode }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0, recordingMode }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-recording').textContent).toContain('disabled');
    });

    byTestId(target, 'session-enable-recording').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-recording').textContent).toContain('always / webm');
    });
    expect(byTestId(target, 'session-detail-action-success').textContent).toContain('enabled');

    byTestId(target, 'session-disable-recording').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-detail-recording').textContent).toContain('disabled');
    });

    const recordingPolicyCalls = fetchImpl.mock.calls
      .filter((call) => String(call[0]).endsWith('/recording-policy'));
    expect(recordingPolicyCalls.map((call) => [call[1]?.method, JSON.parse(String(call[1]?.body)).mode])).toEqual([
      ['PUT', 'always'],
      ['PUT', 'disabled'],
    ]);
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
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure' | 'authConfig'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: overrides.authConfig ?? null,
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
