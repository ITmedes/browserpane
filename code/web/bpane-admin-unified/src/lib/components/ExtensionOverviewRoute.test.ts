import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ExtensionOverviewRoute from './ExtensionOverviewRoute.svelte';

beforeEach(() => window.history.replaceState(null, '', 'http://localhost:3000/admin-new/extensions'));
afterEach(async () => { vi.unstubAllGlobals(); await cleanupRenderedComponents(); });

describe('ExtensionOverviewRoute', () => {
  it('loads extensions through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({ extensions: [extensionPayload()] }, 200));
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(ExtensionOverviewRoute, { authContext: authContext() });

    await vi.waitFor(() => expect(byTestId(target, 'extensions-list').textContent).toContain('Login helper'));
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(ExtensionOverviewRoute, { authContext: authContext(onAuthenticationFailure) });
    await vi.waitFor(() => expect(byTestId(target, 'extensions-error')).toBeInstanceOf(HTMLElement));
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });
});

function authContext(onAuthenticationFailure = vi.fn()): UnifiedAdminContext {
  return {
    auth: { configured: true, authenticated: true, username: 'demo', accessToken: 'token', claims: null },
    authConfig: null,
    accessTokenProvider: async () => 'shell-token',
    onAuthenticationFailure,
    login: async () => {},
    logout: async () => {},
  };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), { status, headers: { 'content-type': 'application/json' } });
}

function extensionPayload() {
  return { id: 'extension-1', name: 'Login helper', description: null, enabled: true, latest_version: null, labels: {}, created_at: '2026-08-07T08:00:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' };
}
