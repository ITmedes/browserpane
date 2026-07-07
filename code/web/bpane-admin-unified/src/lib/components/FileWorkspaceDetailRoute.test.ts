import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceDetailRoute from './FileWorkspaceDetailRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/files/workspaces/workspace-1');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('FileWorkspaceDetailRoute', () => {
  it('loads a file workspace and deletes a workspace file through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/file-workspaces/workspace-1') && init?.method === 'GET') {
        return jsonResponse(workspacePayload(), 200);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files') && init?.method === 'GET') {
        return jsonResponse({ files: [filePayload()] }, 200);
      }
      if (url.endsWith('/api/v1/file-workspaces/workspace-1/files/file-1') && init?.method === 'DELETE') {
        return jsonResponse(filePayload(), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(FileWorkspaceDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'file-workspace-inspector').textContent).toContain('Support inputs');
    });
    byTestId(target, 'file-workspace-file-delete').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'file-workspace-action-success').textContent).toContain('Deleted fixture.csv');
    });
    expect(byTestId(target, 'file-workspace-files-empty').textContent).toContain('No files have been uploaded');
    const deleteCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/file-workspaces/workspace-1/files/file-1') && call[1]?.method === 'DELETE');
    const headers = deleteCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(FileWorkspaceDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'file-workspace-detail-error').textContent).toContain('File workspace detail unavailable');
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

function workspacePayload() {
  return {
    id: 'workspace-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support inputs',
    description: 'Reusable files',
    labels: { team: 'support' },
    files_path: '/api/v1/file-workspaces/workspace-1/files',
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
    provenance: { source: 'test' },
    content_path: '/api/v1/file-workspaces/workspace-1/files/file-1/content',
    created_at: '2026-06-20T09:30:00.000Z',
    updated_at: '2026-06-20T09:30:00.000Z',
  };
}
