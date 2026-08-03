import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import FileWorkspaceCreateRoute from './FileWorkspaceCreateRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/files/workspaces/new');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('FileWorkspaceCreateRoute', () => {
  it('loads project options and creates a project-scoped file workspace', async () => {
    const navigateToWorkspace = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/projects')) {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/file-workspaces') && init?.method === 'POST') {
        return jsonResponse(workspacePayload(), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(FileWorkspaceCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToWorkspace,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'file-workspace-edit-form')).toBeInstanceOf(HTMLElement);
    });
    await input(target, 'file-workspace-edit-name', 'Support inputs');
    await input(target, 'file-workspace-edit-labels', 'team=support');
    await select(target, 'file-workspace-edit-project-binding', 'project');
    await vi.waitFor(() => {
      const projectSelect = byTestId(target, 'file-workspace-edit-project-id') as HTMLSelectElement;
      expect(Array.from(projectSelect.options, (option) => option.value)).toContain('project-1');
    });
    await select(target, 'file-workspace-edit-project-id', 'project-1');
    byTestId(target, 'file-workspace-edit-save').click();

    await vi.waitFor(() => {
      expect(navigateToWorkspace).toHaveBeenCalledWith(expect.objectContaining({ id: 'workspace-1' }));
    });
    const createCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/file-workspaces') && call[1]?.method === 'POST');
    expect(createCall?.[1]?.body).toBe(JSON.stringify({
      project_id: 'project-1',
      name: 'Support inputs',
      description: null,
      labels: { team: 'support' },
    }));
    const headers = createCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

async function select(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLSelectElement;
  element.value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
  await tick();
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

function projectListPayload() {
  return {
    projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
  };
}

function workspacePayload() {
  return {
    id: 'workspace-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support inputs',
    description: null,
    labels: { team: 'support' },
    files_path: '/api/v1/file-workspaces/workspace-1/files',
    created_at: '2026-06-20T09:00:00.000Z',
    updated_at: '2026-06-20T10:00:00.000Z',
  };
}
