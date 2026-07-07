import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import DashboardOverviewRoute from './DashboardOverviewRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('DashboardOverviewRoute', () => {
  it('loads dashboard catalogs through authenticated clients', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const path = new URL(String(input)).pathname;
      if (path === '/api/v1/sessions') {
        return jsonResponse({ sessions: [sessionPayload({ id: 'session-1', stopAllowed: true })] }, 200);
      }
      if (path === '/api/v1/sessions/session-1/recordings') {
        return jsonResponse({ recordings: [recordingPayload()] }, 200);
      }
      if (path === '/api/v1/projects') {
        return jsonResponse({ projects: [] }, 200);
      }
      if (path === '/api/v1/browser-contexts') {
        return jsonResponse({ contexts: [] }, 200);
      }
      if (path === '/api/v1/egress-profiles') {
        return jsonResponse({ profiles: [] }, 200);
      }
      if (path === '/api/v1/file-workspaces') {
        return jsonResponse({ workspaces: [] }, 200);
      }
      if (path === '/api/v1/workflows') {
        return jsonResponse({ workflows: [] }, 200);
      }
      if (path === '/api/v1/workflow-runs') {
        return jsonResponse({ runs: [] }, 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(DashboardOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'dashboard-link-recordings').textContent).toContain('1');
    });
    expect(byTestId(target, 'dashboard-metric-sessions').textContent).toContain('1 total');
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('renders partial catalog failures without dropping the dashboard', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const path = new URL(String(input)).pathname;
      if (path === '/api/v1/egress-profiles') {
        return new Response('unavailable', { status: 503 });
      }
      return jsonResponse(emptyCatalogPayload(path), 200);
    }));
    const target = renderComponent(DashboardOverviewRoute, {
      authContext: authContext(),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'dashboard-partial-warning').textContent).toContain('Egress profiles');
    });
    expect(byTestId(target, 'dashboard-overview').textContent).toContain('Catalog shortcuts');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(DashboardOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'dashboard-error').textContent).toContain('Dashboard unavailable');
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

function emptyCatalogPayload(path: string): Record<string, unknown> {
  if (path === '/api/v1/sessions') {
    return { sessions: [] };
  }
  if (path === '/api/v1/projects') {
    return { projects: [] };
  }
  if (path === '/api/v1/browser-contexts') {
    return { contexts: [] };
  }
  if (path === '/api/v1/file-workspaces') {
    return { workspaces: [] };
  }
  if (path === '/api/v1/workflows') {
    return { workflows: [] };
  }
  if (path === '/api/v1/workflow-runs') {
    return { runs: [] };
  }
  return {};
}

function recordingPayload(): Record<string, unknown> {
  return {
    id: 'recording-1',
    session_id: 'session-1',
    previous_recording_id: null,
    state: 'ready',
    format: 'webm',
    mime_type: 'video/webm',
    bytes: 512,
    duration_ms: 3000,
    error: null,
    termination_reason: null,
    artifact_available: true,
    content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
    started_at: '2026-06-21T10:02:00.000Z',
    completed_at: '2026-06-21T10:05:00.000Z',
    created_at: '2026-06-21T10:02:00.000Z',
    updated_at: '2026-06-21T10:12:00.000Z',
  };
}
