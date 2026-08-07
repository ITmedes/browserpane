import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload, sessionStatusPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionPolicyRoute from './SessionPolicyRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionPolicyRoute', () => {
  it('loads effective capabilities and supporting project policy evidence', async () => {
    const payload = sessionPayload();
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload());
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse({
          ...payload,
          capabilities: {
            ...(payload.capabilities as Record<string, unknown>),
            file_transfer: false,
          },
        });
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return jsonResponse(projectPayload());
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionPolicyRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-policy-scope').textContent).toContain('Support project policy');
    });
    expect(byTestId(target, 'session-subarea-policy').getAttribute('aria-current')).toBe('page');
    expect(byTestId(target, 'session-policy-capability-file_transfer').textContent).toContain('Disabled');
    expect(byTestId(target, 'session-policy-capability-file_transfer-description').textContent)
      .toContain('project blocks browser uploads');
    expect(byTestId(target, 'session-policy-browser-upload').textContent).toContain('Blocked');
    expect(byTestId(target, 'session-policy-local-file-mode').textContent).toContain('Startup-enforced');
  });

  it('keeps effective session facts visible when project evidence is unavailable', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1/status')) {
        return jsonResponse(sessionStatusPayload());
      }
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload());
      }
      if (url.endsWith('/api/v1/projects/project-1')) {
        return new Response('unavailable', { status: 503 });
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(SessionPolicyRoute, {
      authContext: authContext(),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-policy-project-warning').textContent).toContain('unavailable');
    });
    expect(byTestId(target, 'session-policy-capability-browser_input').textContent).toContain('Enabled');
    expect(byTestId(target, 'session-policy-scope').textContent).toContain('Owner-scoped defaults');
  });

  it('delegates authentication failures to the shared shell handler', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(SessionPolicyRoute, {
      authContext: authContext({ onAuthenticationFailure }),
      sessionId: 'session-1',
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-policy-error').textContent).toContain('unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
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

function projectPayload() {
  return {
    id: 'project-1',
    name: 'Support',
    description: 'Support browser work',
    labels: {},
    quotas: {},
    policy: {
      allowed_session_template_ids: ['template-1'],
      allowed_egress_profile_ids: ['egress-1'],
      allowed_extension_ids: [],
      allowed_browser_context_ids: ['context-1'],
      allowed_file_workspace_ids: ['workspace-1'],
      allow_browser_uploads: false,
      allow_browser_downloads: true,
      allow_session_file_bindings: false,
      allow_manual_recordings: false,
      usage_budget_enforcement: 'block_session_creation',
    },
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
