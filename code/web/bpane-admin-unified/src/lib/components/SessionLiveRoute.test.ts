import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload, sessionStatusPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionLiveRoute from './SessionLiveRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions/session-1/live');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  await cleanupRenderedComponents();
});

describe('SessionLiveRoute', () => {
  it('loads live status, supports refresh, and opens the standalone preview', async () => {
    let statusLoads = 0;
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      expect(new Headers(init?.headers).get('authorization')).toBe('Bearer shell-token');
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        statusLoads += 1;
        return jsonResponse(sessionStatusPayload({ totalClients: statusLoads }), 200);
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: statusLoads }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const focus = vi.fn();
    const open = vi.spyOn(window, 'open').mockReturnValue({ focus } as unknown as Window);
    const target = renderComponent(SessionLiveRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-clients').textContent).toContain('1');
    });
    expect(byTestId(target, 'session-subarea-live').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-subarea-overview').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1');

    byTestId(target, 'session-live-refresh').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-clients').textContent).toContain('2');
    });
    expect(byTestId(target, 'session-live-action-success').textContent).toContain('refreshed');

    byTestId(target, 'session-live-connect').click();
    expect(open).toHaveBeenCalledWith(
      '/admin-new/sessions/session-1/preview',
      'bpane-session-preview-session-1',
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    expect(focus).toHaveBeenCalledOnce();
  });

  it('keeps the session usable when best-effort live status is unavailable', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/status')) {
        return new Response('runtime not ready', { status: 409 });
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }), 200);
      }
      return new Response('not found', { status: 404 });
    }));
    const target = renderComponent(SessionLiveRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-panel')).toBeTruthy();
    });
    expect(byTestId(target, 'session-live-resolution').textContent).toContain('not reported');
    expect((byTestId(target, 'session-live-connect') as HTMLButtonElement).disabled).toBe(false);
  });

  it('reports popup blocking without leaving the route', async () => {
    vi.spyOn(window, 'open').mockReturnValue(null);
    vi.stubGlobal('fetch', liveFetch());
    const target = renderComponent(SessionLiveRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-panel')).toBeTruthy();
    });
    byTestId(target, 'session-live-connect').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-action-error').textContent).toContain('popup was blocked');
    });
  });

  it('delegates authentication failures to the shared shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionLiveRoute, {
      authContext: authContext({ onAuthenticationFailure }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-error').textContent).toContain('Live session unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('rejects a route without a live session id before issuing requests', async () => {
    window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions');
    const fetchImpl = vi.fn<typeof fetch>();
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionLiveRoute, { authContext: authContext() });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-live-error').textContent).toContain('Session id is missing');
    });
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

function liveFetch(): ReturnType<typeof vi.fn<typeof fetch>> {
  return vi.fn<typeof fetch>(async (input) => {
    const url = String(input);
    if (url.endsWith('/api/v1/sessions/session-1/status')) {
      return jsonResponse(sessionStatusPayload({ totalClients: 0 }), 200);
    }
    if (url.endsWith('/api/v1/sessions/session-1')) {
      return jsonResponse(sessionPayload({ totalClients: 0 }), 200);
    }
    return new Response('not found', { status: 404 });
  });
}

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
