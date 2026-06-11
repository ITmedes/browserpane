import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ProjectOverviewRoute from './ProjectOverviewRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('ProjectOverviewRoute', () => {
  it('loads projects with the authenticated shell token provider', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse(projectListPayload(), 200));
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(ProjectOverviewRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'projects-list').textContent).toContain('Support');
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:3000/api/v1/projects'),
      expect.objectContaining({
        headers: expect.objectContaining({
          authorization: 'Bearer shell-token',
        }),
      }),
    );
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(ProjectOverviewRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'projects-error').textContent).toContain('Project catalog unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
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
    projects: [{
      id: '11111111-1111-4111-8111-111111111111',
      name: 'Support',
      description: 'Support browser work',
      labels: {},
      quotas: {},
      policy: {},
      state: 'active',
      usage: {
        project_id: '11111111-1111-4111-8111-111111111111',
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
    }],
  };
}
