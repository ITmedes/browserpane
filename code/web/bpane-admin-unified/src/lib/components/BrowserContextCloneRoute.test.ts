import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextCloneRoute from './BrowserContextCloneRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/browser-contexts/context-1/clone');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('BrowserContextCloneRoute', () => {
  it('loads the source and creates a reusable clone through the authenticated API', async () => {
    const navigateToContext = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/browser-contexts/context-1') && init?.method === 'GET') {
        return jsonResponse(browserContextPayload(), 200);
      }
      if (url.endsWith('/api/v1/projects') && init?.method === 'GET') {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts/context-1/clone') && init?.method === 'POST') {
        return jsonResponse(browserContextPayload({ id: 'context-copy', name: 'Support baseline copy' }), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextCloneRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToContext,
    });

    await vi.waitFor(() => {
      expect((byTestId(target, 'browser-context-edit-name') as HTMLInputElement).value).toBe(
        'Support baseline copy',
      );
    });
    const persistence = byTestId(target, 'browser-context-edit-persistence-mode') as HTMLSelectElement;
    expect(persistence.value).toBe('reusable');
    expect(persistence.disabled).toBe(true);
    byTestId(target, 'browser-context-edit-save').click();

    await vi.waitFor(() => {
      expect(navigateToContext).toHaveBeenCalledWith(expect.objectContaining({ id: 'context-copy' }));
    });
    const cloneCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/browser-contexts/context-1/clone') && call[1]?.method === 'POST');
    expect(cloneCall?.[1]?.body).toBe(JSON.stringify({
      project_id: 'project-1',
      name: 'Support baseline copy',
      description: 'Reusable support profile',
      labels: { team: 'support' },
      retention_sec: 604800,
      max_profile_storage_bytes: 1073741824,
    }));
    const headers = cloneCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('blocks cloning while the reusable context has an active writer', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/browser-contexts/context-1')) {
        return jsonResponse(browserContextPayload({
          activeRuntimeSessionCount: 1,
          activeRuntimeSessionId: 'session-1',
        }), 200);
      }
      if (url.endsWith('/api/v1/projects')) {
        return jsonResponse(projectListPayload(), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextCloneRoute, {
      authContext: authContext(),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-clone-blocked').textContent).toContain(
        'Stop the active browser session',
      );
    });
    expect(byTestId(target, 'browser-context-clone-active-session').getAttribute('href')).toBe(
      '/admin-new/sessions/session-1',
    );
    expect(target.querySelector('[data-testid="browser-context-edit-form"]')).toBeNull();
    expect(fetchImpl.mock.calls.some((call) => String(call[0]).endsWith('/clone'))).toBe(false);
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

function projectListPayload() {
  return {
    projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
  };
}

function browserContextPayload(overrides: Partial<{
  readonly id: string;
  readonly name: string;
  readonly activeRuntimeSessionCount: number;
  readonly activeRuntimeSessionId: string | null;
}> = {}) {
  return {
    id: overrides.id ?? 'context-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: overrides.name ?? 'Support baseline',
    description: 'Reusable support profile',
    labels: { team: 'support' },
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: 1073741824,
    state: 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: overrides.activeRuntimeSessionCount ?? 0,
      active_runtime_session_id: overrides.activeRuntimeSessionId ?? null,
      profile_storage_bytes: 1048576,
      profile_storage_limit_exceeded: false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
