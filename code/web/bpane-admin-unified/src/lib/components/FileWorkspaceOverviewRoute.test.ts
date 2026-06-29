import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceOverviewRoute from './FileWorkspaceOverviewRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('FileWorkspaceOverviewRoute', () => {
  it('loads workspaces and best-effort file counts through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/file-workspaces')) {
        return jsonResponse({
          workspaces: [
            workspacePayload({ id: 'workspace-1', name: 'Support inputs' }),
            workspacePayload({ id: 'workspace-2', name: 'Archived export' }),
          ],
        }, 200);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files')) {
        return jsonResponse({ files: [filePayload()] }, 200);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-2/files')) {
        return new Response('temporary failure', { status: 503 });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(FileWorkspaceOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'file-workspaces-list').textContent).toContain('Support inputs');
    });
    expect(byTestId(target, 'file-workspaces-metric-files').textContent).toContain('1');
    expect(byTestId(target, 'file-workspaces-list').textContent).toContain('files unavailable');
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(FileWorkspaceOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'file-workspaces-error').textContent).toContain('File workspace catalog request failed');
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

function workspacePayload(overrides: Partial<{ readonly id: string; readonly name: string }> = {}) {
  return {
    id: overrides.id ?? 'workspace-1',
    project_id: null,
    project: null,
    name: overrides.name ?? 'Support inputs',
    description: null,
    labels: {},
    files_path: `/api/v1/file-workspaces/${overrides.id ?? 'workspace-1'}/files`,
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}

function filePayload() {
  return {
    id: 'file-1',
    workspace_id: 'workspace-1',
    name: 'fixture.csv',
    media_type: 'text/csv',
    byte_count: 18,
    sha256_hex: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
    provenance: null,
    content_path: '/api/v1/file-workspaces/workspace-1/files/file-1/content',
    created_at: '2026-06-20T09:30:00.000Z',
    updated_at: '2026-06-20T09:30:00.000Z',
  };
}
