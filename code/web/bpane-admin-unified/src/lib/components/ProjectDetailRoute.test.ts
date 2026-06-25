import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectDetailRoute from './ProjectDetailRoute.svelte';

const PROJECT_ID = '11111111-1111-4111-8111-111111111111';

beforeEach(() => {
  window.history.replaceState(null, '', `http://localhost:3000/admin-new/projects/${PROJECT_ID}`);
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('ProjectDetailRoute', () => {
  it('loads, refreshes usage, and saves an existing project through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = input.toString();
      if (init?.method === 'PUT') {
        return jsonResponse({
          ...projectPayload(),
          description: 'Updated support browser work',
          labels: { team: 'support', env: 'prod' },
        }, 200);
      }
      if (url.endsWith('/usage')) {
        return jsonResponse({
          ...projectPayload().usage,
          session_creations: 2,
        }, 200);
      }
      if (url.endsWith('/session-templates')) {
        return jsonResponse({
          templates: [{
            id: 'template-support',
            name: 'Support Browser',
            description: 'Approved support defaults',
            labels: {},
            defaults: {},
            version: 1,
            created_at: '2026-06-11T09:00:00.000Z',
            updated_at: '2026-06-11T10:00:00.000Z',
          }],
        }, 200);
      }
      if (url.endsWith('/browser-contexts')) {
        return jsonResponse({
          contexts: [{
            id: '22222222-2222-4222-8222-222222222222',
            project_id: null,
            project: null,
            name: 'Support Context',
            description: null,
            labels: {},
            persistence_mode: 'reusable',
            retention_sec: null,
            retention_expires_at: null,
            max_profile_storage_bytes: null,
            state: 'ready',
            usage: {},
            created_at: '2026-06-11T09:00:00.000Z',
            updated_at: '2026-06-11T10:00:00.000Z',
            last_used_at: null,
            deleted_at: null,
          }],
        }, 200);
      }
      if (url.endsWith('/egress-profiles')) {
        return jsonResponse({ profiles: [] }, 200);
      }
      if (url.endsWith('/extensions')) {
        return jsonResponse({ extensions: [] }, 200);
      }
      if (url.endsWith('/file-workspaces')) {
        return jsonResponse({ workspaces: [] }, 200);
      }
      return jsonResponse(projectPayload(), 200);
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(ProjectDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-detail-route').textContent).toContain('Edit project');
    });
    byTestId(target, 'project-refresh-usage').click();
    await vi.waitFor(() => {
      expect(byTestId(target, 'project-action-success').textContent).toContain('Project usage refreshed');
    });

    const description = byTestId(target, 'project-edit-description') as HTMLTextAreaElement;
    description.value = 'Updated support browser work';
    description.dispatchEvent(new Event('input', { bubbles: true }));
    const labels = byTestId(target, 'project-edit-labels') as HTMLTextAreaElement;
    labels.value = 'env=prod\nteam=support';
    labels.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();
    byTestId(target, 'project-edit-save').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-action-success').textContent).toContain('Project saved');
    });
    const putCall = fetchImpl.mock.calls.find((call) => call[1]?.method === 'PUT');
    expect(putCall?.[0]).toEqual(new URL(`http://localhost:3000/api/v1/projects/${PROJECT_ID}`));
    expect(JSON.parse(putCall?.[1]?.body as string)).toMatchObject({
      name: 'Support',
      description: 'Updated support browser work',
      labels: { env: 'prod', team: 'support' },
      quotas: {},
      policy: expect.objectContaining({ allow_browser_uploads: true }),
      state: 'active',
    });
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(ProjectDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'project-detail-error').textContent).toContain('Project detail unavailable');
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

function projectPayload() {
  return {
    id: PROJECT_ID,
    name: 'Support',
    description: 'Support browser work',
    labels: {},
    quotas: {},
    policy: {},
    state: 'active',
    usage: {
      project_id: PROJECT_ID,
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
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
