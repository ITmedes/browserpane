import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionFilesRoute from './SessionFilesRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionFilesRoute', () => {
  it('loads session policy and composes both file evidence areas', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      expect(new Headers(init?.headers).get('authorization')).toBe('Bearer owner-token');
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }));
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return jsonResponse(projectPayload());
      }
      if (url.endsWith('/files')) {
        return jsonResponse({ files: [] });
      }
      if (url.endsWith('/file-bindings')) {
        return jsonResponse({ bindings: [] });
      }
      if (url.endsWith('/file-workspaces')) {
        return jsonResponse({ workspaces: [] });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionFilesRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => expect(byTestId(target, 'session-transfer-files-panel')).toBeTruthy());
    expect(byTestId(target, 'session-file-bindings-panel')).toBeTruthy();
    expect(byTestId(target, 'session-subarea-files').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-subarea-live').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/live');
    expect(byTestId(target, 'session-files-empty')).toBeTruthy();
    expect(byTestId(target, 'session-file-bindings-empty')).toBeTruthy();
    expect(byTestId(target, 'session-files-project-name').textContent).toContain('Support');
    expect(byTestId(target, 'session-files-project-link').getAttribute('href'))
      .toBe('/admin-new/projects/project-1');
  });

  it('shows project policy blocking while preserving read-only evidence', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }));
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return jsonResponse(projectPayload({
          allow_browser_uploads: false,
          allow_session_file_bindings: false,
        }));
      }
      if (url.endsWith('/files')) {
        return jsonResponse({ files: [] });
      }
      if (url.endsWith('/file-bindings')) {
        return jsonResponse({ bindings: [] });
      }
      if (url.endsWith('/file-workspaces')) {
        return jsonResponse({ workspaces: [] });
      }
      return new Response('not found', { status: 404 });
    }));
    const target = renderComponent(SessionFilesRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-file-bindings-policy-blocked').textContent)
        .toContain('project blocks session file bindings');
    });
    expect(byTestId(target, 'session-files-policy-blocked').textContent)
      .toContain('project blocks browser uploads');
    expect((byTestId(target, 'session-file-binding-create') as HTMLButtonElement).disabled).toBe(true);
  });

  it('keeps captured-file evidence available when project policy loading fails', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload({ totalClients: 0 }));
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return new Response('project unavailable', { status: 503 });
      }
      if (url.endsWith('/files')) {
        return jsonResponse({ files: [] });
      }
      if (url.endsWith('/file-bindings')) {
        return jsonResponse({ bindings: [] });
      }
      if (url.endsWith('/file-workspaces')) {
        return jsonResponse({ workspaces: [] });
      }
      return new Response('not found', { status: 404 });
    }));
    const target = renderComponent(SessionFilesRoute, { authContext: authContext(), sessionId: 'session-1' });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-files-project-warning').textContent).toContain('HTTP 503');
    });
    expect(byTestId(target, 'session-files-empty')).toBeTruthy();
    expect((byTestId(target, 'session-file-binding-create') as HTMLButtonElement).disabled).toBe(true);
    expect(byTestId(target, 'session-files-project-link').getAttribute('href'))
      .toBe('/admin-new/projects/project-1');
  });

  it('delegates session authentication failure to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionFilesRoute, {
      authContext: authContext({ onAuthenticationFailure }),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-files-route-error').textContent).toContain('HTTP 401');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
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

function projectPayload(policy: Readonly<Record<string, unknown>> = {}) {
  return {
    id: 'project-1',
    name: 'Support',
    description: 'Support browser work',
    labels: {},
    quotas: {},
    policy,
    state: 'active',
    usage: {
      project_id: 'project-1',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 30_000,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-08-07T10:00:00Z',
    },
    created_at: '2026-08-07T09:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
