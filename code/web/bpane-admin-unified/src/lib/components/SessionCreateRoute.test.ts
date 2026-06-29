import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionCreateRoute from './SessionCreateRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions/new');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionCreateRoute', () => {
  it('loads options, waits for form submit, and creates the session through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/projects') && init?.method === 'GET') {
        return jsonResponse({ projects: [projectPayload()] }, 200);
      }
      if (url.endsWith('/api/v1/session-templates')) {
        return jsonResponse({ templates: [policyOptionPayload({ id: 'template-1', name: 'Support Template' })] }, 200);
      }
      if (url.endsWith('/api/v1/browser-contexts')) {
        return jsonResponse({ contexts: [policyOptionPayload({ id: 'context-1', name: 'Support Context' })] }, 200);
      }
      if (url.endsWith('/api/v1/egress-profiles')) {
        return jsonResponse({ profiles: [policyOptionPayload({ id: 'egress-1', name: 'Proxy Egress' })] }, 200);
      }
      if (url.endsWith('/api/v1/extensions')) {
        return jsonResponse({ extensions: [] }, 200);
      }
      if (url.endsWith('/api/v1/file-workspaces')) {
        return jsonResponse({ workspaces: [] }, 200);
      }
      if (url.endsWith('/api/v1/sessions') && init?.method === 'POST') {
        return jsonResponse(sessionPayload({ id: 'created-session', totalClients: 0 }), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const navigateToSession = vi.fn();
    const target = renderComponent(SessionCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToSession,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-create-form').textContent).toContain('Support Template');
    });
    expect(fetchImpl.mock.calls.some((call) => call[1]?.method === 'POST')).toBe(false);

    setSelectValue(byTestId(target, 'session-create-project-id'), 'project-1');
    setSelectValue(byTestId(target, 'session-create-template-id'), 'template-1');
    setSelectValue(byTestId(target, 'session-create-browser-context-mode'), 'reusable');
    setSelectValue(byTestId(target, 'session-create-browser-context-id'), 'context-1');
    setSelectValue(byTestId(target, 'session-create-egress-profile-id'), 'egress-1');
    setInputValue(byTestId(target, 'session-create-labels'), 'team=support');
    await tick();
    byTestId(target, 'session-create-save').click();

    await vi.waitFor(() => {
      expect(navigateToSession).toHaveBeenCalledWith(expect.objectContaining({ id: 'created-session' }));
    });
    const createCall = fetchImpl.mock.calls.find((call) => String(call[0]).endsWith('/api/v1/sessions') && call[1]?.method === 'POST');
    expect(createCall?.[0]).toEqual(new URL('http://localhost:3000/api/v1/sessions'));
    expect(JSON.parse(createCall?.[1]?.body as string)).toEqual({
      project_id: 'project-1',
      template_id: 'template-1',
      browser_context: { mode: 'reusable', context_id: 'context-1' },
      labels: { team: 'support' },
      network_identity: { egress_profile_id: 'egress-1' },
    });
    expect(createCall?.[1]?.body).not.toContain('bpane_admin_surface');
    const headers = createCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('reports create API errors locally', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions') && init?.method === 'POST') {
        return new Response('conflict', { status: 409 });
      }
      if (url.endsWith('/api/v1/projects')) {
        return jsonResponse({ projects: [] }, 200);
      }
      if (url.endsWith('/api/v1/session-templates')) {
        return jsonResponse({ templates: [] }, 200);
      }
      if (url.endsWith('/api/v1/browser-contexts')) {
        return jsonResponse({ contexts: [] }, 200);
      }
      if (url.endsWith('/api/v1/egress-profiles')) {
        return jsonResponse({ profiles: [] }, 200);
      }
      if (url.endsWith('/api/v1/extensions')) {
        return jsonResponse({ extensions: [] }, 200);
      }
      return jsonResponse({ workspaces: [] }, 200);
    }));
    const target = renderComponent(SessionCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToSession: vi.fn(),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-create-route').textContent).toContain('New session settings');
    });
    byTestId(target, 'session-create-save').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-create-error').textContent).toContain('Session action failed');
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

function setSelectValue(element: Element, value: string): void {
  (element as HTMLSelectElement).value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function policyOptionPayload(overrides: Record<string, unknown> = {}) {
  return {
    id: 'option-1',
    name: 'Option',
    description: null,
    state: 'ready',
    ...overrides,
  };
}

function projectPayload() {
  return {
    id: 'project-1',
    name: 'Support',
    description: null,
    labels: {},
    quotas: {},
    policy: {},
    state: 'active',
    usage: {
      project_id: 'project-1',
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
      observed_at: '2026-06-29T12:00:00Z',
    },
    created_at: '2026-06-29T12:00:00Z',
    updated_at: '2026-06-29T12:00:00Z',
  };
}
