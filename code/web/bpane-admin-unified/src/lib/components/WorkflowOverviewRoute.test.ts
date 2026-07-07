import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowOverviewRoute from './WorkflowOverviewRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('WorkflowOverviewRoute', () => {
  it('loads visible workflows, hides smoke definitions, and loads latest version metadata', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows') && init?.method === 'GET') {
        return jsonResponse({
          workflows: [
            workflowPayload({ id: 'tour', name: 'BrowserPane Tour', template: true }),
            workflowPayload({ id: 'support', name: 'Support audit' }),
            workflowPayload({ id: 'hidden', name: 'Smoke workflow', smoke: true }),
          ],
        }, 200);
      }
      if (url.endsWith('/api/v1/workflows/tour/versions/v1')) {
        return jsonResponse(versionPayload({ workflowId: 'tour' }), 200);
      }
      if (url.endsWith('/api/v1/workflows/support/versions/v1')) {
        return jsonResponse(versionPayload({ workflowId: 'support' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflows-list').textContent).toContain('BrowserPane Tour');
    });
    expect(byTestId(target, 'workflows-list').textContent).toContain('Support audit');
    expect(byTestId(target, 'workflows-list').textContent).not.toContain('Smoke workflow');
    expect(byTestId(target, 'workflows-hidden-note').textContent).toContain('1 internal');
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('auto-registers the BrowserPane Tour template when missing', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/workflows') && init?.method === 'GET') {
        return jsonResponse({ workflows: [] }, 200);
      }
      if (url.endsWith('/api/v1/workflows') && init?.method === 'POST') {
        return jsonResponse(workflowPayload({ id: 'tour', name: 'BrowserPane Tour', template: true, latest: null }), 201);
      }
      if (url.endsWith('/api/v1/workflows/tour/versions') && init?.method === 'POST') {
        return jsonResponse(versionPayload({ workflowId: 'tour' }), 201);
      }
      if (url.endsWith('/api/v1/workflows/tour') && init?.method === 'GET') {
        return jsonResponse(workflowPayload({ id: 'tour', name: 'BrowserPane Tour', template: true }), 200);
      }
      if (url.endsWith('/api/v1/workflows/tour/versions/v1')) {
        return jsonResponse(versionPayload({ workflowId: 'tour' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflows-list').textContent).toContain('BrowserPane Tour');
    });
    expect(fetchImpl.mock.calls.some((call) =>
      String(call[0]).endsWith('/api/v1/workflows') && call[1]?.method === 'POST')).toBe(true);
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(WorkflowOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflows-error').textContent).toContain('Workflow catalog request failed');
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

function workflowPayload(options: {
  readonly id: string;
  readonly name: string;
  readonly template?: boolean;
  readonly smoke?: boolean;
  readonly latest?: string | null;
}) {
  return {
    id: options.id,
    name: options.name,
    description: null,
    labels: {
      ...(options.template ? { bpane_admin_template: 'browserpane-tour' } : {}),
      ...(options.smoke ? { suite: 'admin-workflow-smoke' } : {}),
    },
    latest_version: options.latest === undefined ? 'v1' : options.latest,
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function versionPayload(options: { readonly workflowId: string }) {
  return {
    id: `${options.workflowId}-version-1`,
    workflow_definition_id: options.workflowId,
    version: 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}
