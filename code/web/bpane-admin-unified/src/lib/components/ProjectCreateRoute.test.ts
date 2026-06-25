import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectCreateRoute from './ProjectCreateRoute.svelte';

const PROJECT_ID = '11111111-1111-4111-8111-111111111111';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/projects/new');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('ProjectCreateRoute', () => {
  it('creates a project through the authenticated API and navigates to the detail route', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = input.toString();
      if (init?.method === 'POST' && url.endsWith('/api/v1/projects')) {
        return jsonResponse({
          ...projectPayload(),
          name: 'Customer Support',
          labels: { team: 'support' },
        }, 201);
      }
      if (url.endsWith('/session-templates')) {
        return jsonResponse({ templates: [] }, 200);
      }
      if (url.endsWith('/browser-contexts')) {
        return jsonResponse({ contexts: [] }, 200);
      }
      if (url.endsWith('/egress-profiles')) {
        return jsonResponse({ profiles: [] }, 200);
      }
      if (url.endsWith('/extensions')) {
        return jsonResponse({ extensions: [] }, 200);
      }
      return jsonResponse({ workspaces: [] }, 200);
    });
    vi.stubGlobal('fetch', fetchImpl);
    const navigateToProject = vi.fn();
    const target = renderComponent(ProjectCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToProject,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-create-route').textContent).toContain('New project settings');
    });
    setInputValue(byTestId(target, 'project-edit-name'), 'Customer Support');
    setInputValue(byTestId(target, 'project-edit-labels'), 'team=support');
    await tick();
    byTestId(target, 'project-edit-save').click();

    await vi.waitFor(() => {
      expect(navigateToProject).toHaveBeenCalledWith(expect.objectContaining({ id: PROJECT_ID }));
    });
    const postCall = fetchImpl.mock.calls.find((call) => call[1]?.method === 'POST');
    expect(postCall?.[0]).toEqual(new URL('http://localhost:3000/api/v1/projects'));
    expect(JSON.parse(postCall?.[1]?.body as string)).toMatchObject({
      name: 'Customer Support',
      labels: { team: 'support' },
      state: 'active',
      policy: expect.objectContaining({ allow_browser_uploads: true }),
    });
  });

  it('reports create API errors locally', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input, init) => {
      if (init?.method === 'POST') {
        return new Response('conflict', { status: 409 });
      }
      return jsonResponse({ templates: [], contexts: [], profiles: [], extensions: [], workspaces: [] }, 200);
    }));
    const target = renderComponent(ProjectCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToProject: vi.fn(),
    });

    setInputValue(byTestId(target, 'project-edit-name'), 'Customer Support');
    await tick();
    byTestId(target, 'project-edit-save').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-create-error').textContent).toContain('Project action failed');
    });
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

function setInputValue(element: Element, value: string): void {
  (element as HTMLInputElement | HTMLTextAreaElement).value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function projectPayload() {
  return {
    id: PROJECT_ID,
    name: 'Support',
    description: null,
    labels: {},
    quotas: {},
    policy: {},
    state: 'active',
    usage: {
      project_id: PROJECT_ID,
      active_sessions: 0,
      queued_sessions: 0,
      session_creations: 0,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 0,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
