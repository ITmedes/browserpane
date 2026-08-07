import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextDetailRoute from './BrowserContextDetailRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/browser-contexts/context-1');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  await cleanupRenderedComponents();
});

describe('BrowserContextDetailRoute', () => {
  it('loads and soft-deletes a browser context through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/browser-contexts/context-1') && init?.method === 'GET') {
        return jsonResponse(browserContextPayload({ state: 'ready' }), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts/context-1') && init?.method === 'DELETE') {
        return jsonResponse(browserContextPayload({ state: 'deleted' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-inspector').textContent).toContain('Support baseline');
    });
    byTestId(target, 'browser-context-delete').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-action-success').textContent).toContain('Browser context deleted');
    });
    const deleteCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/browser-contexts/context-1') && call[1]?.method === 'DELETE');
    const headers = deleteCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(BrowserContextDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-detail-error').textContent).toContain('Browser context detail unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });

  it('downloads and revokes an exported browser context archive', async () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:context-export');
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/browser-contexts/context-1') && init?.method === 'GET') {
        return jsonResponse(browserContextPayload(), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts/context-1/export') && init?.method === 'GET') {
        return new Response('zip-bytes', {
          status: 200,
          headers: {
            'content-type': 'application/zip',
            'content-disposition': 'attachment; filename="support-baseline.zip"',
          },
        });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-export')).toBeInstanceOf(HTMLButtonElement);
    });
    byTestId(target, 'browser-context-export').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-action-success').textContent).toContain(
        'support-baseline.zip',
      );
    });
    expect(createObjectURL).toHaveBeenCalledWith(expect.any(Blob));
    expect(click).toHaveBeenCalledOnce();
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:context-export');
    const exportCall = fetchImpl.mock.calls.find((call) => String(call[0]).endsWith('/export'));
    const headers = exportCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('keeps the detail view available when export is rejected by current runtime state', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/browser-contexts/context-1') && init?.method === 'GET') {
        return jsonResponse(browserContextPayload(), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts/context-1/export') && init?.method === 'GET') {
        return jsonResponse({ error: 'context became active' }, 409);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextDetailRoute, {
      authContext: authContext(),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-export')).toBeInstanceOf(HTMLButtonElement);
    });
    byTestId(target, 'browser-context-export').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-action-error').textContent).toContain('HTTP 409');
    });
    expect(byTestId(target, 'browser-context-detail-name').textContent).toContain('Support baseline');
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

function browserContextPayload(overrides: Partial<{ readonly state: 'ready' | 'deleted' }> = {}) {
  return {
    id: 'context-1',
    project_id: null,
    project: null,
    name: 'Support baseline',
    description: null,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: null,
    state: overrides.state ?? 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: null,
      profile_storage_limit_exceeded: false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: overrides.state === 'deleted' ? '2026-06-18T11:00:00.000Z' : null,
  };
}
