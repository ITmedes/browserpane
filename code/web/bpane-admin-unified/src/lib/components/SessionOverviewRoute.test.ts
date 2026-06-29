import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionOverviewRoute from './SessionOverviewRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionOverviewRoute', () => {
  it('loads sessions and exposes the new-session form route without creating a session', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions') && init?.method === 'GET') {
        return jsonResponse({ sessions: [sessionPayload()] }, 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'sessions-list').textContent).toContain('session-1');
    });
    expect(byTestId(target, 'sessions-new').getAttribute('href')).toBe('/admin-new/sessions/new');
    expect(fetchImpl.mock.calls.some((call) => call[1]?.method === 'POST')).toBe(false);
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'sessions-error').textContent).toContain('Session catalog unavailable');
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
