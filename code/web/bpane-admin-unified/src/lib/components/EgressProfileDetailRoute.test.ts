import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import EgressProfileDetailRoute from './EgressProfileDetailRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/egress/profile-1');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('EgressProfileDetailRoute', () => {
  it('loads a profile and saves edits through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/egress-profiles/profile-1') && init?.method === 'GET') {
        return jsonResponse(egressProfilePayload({ name: 'Direct profile' }), 200);
      }
      if (url.endsWith('/api/v1/projects')) {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/egress-profiles/profile-1') && init?.method === 'PUT') {
        return jsonResponse(egressProfilePayload({ name: 'Renamed profile' }), 200);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(EgressProfileDetailRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'egress-profile-inspector').textContent).toContain('Direct profile');
    });
    await input(target, 'egress-profile-edit-name', 'Renamed profile');
    byTestId(target, 'egress-profile-edit-save').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'egress-profile-action-success').textContent).toContain('Egress profile saved');
    });
    const updateCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/egress-profiles/profile-1') && call[1]?.method === 'PUT');
    expect(updateCall?.[1]?.body).toContain('"name":"Renamed profile"');
    const headers = updateCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });

  it('delegates authentication failures back to the shell', async () => {
    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const target = renderComponent(EgressProfileDetailRoute, {
      authContext: authContext({
        accessTokenProvider: async () => 'expired-token',
        onAuthenticationFailure,
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'egress-profile-detail-error').textContent).toContain('Egress profile detail unavailable');
    });
    expect(onAuthenticationFailure).toHaveBeenCalled();
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
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

function egressProfilePayload(overrides: Partial<{ readonly name: string }> = {}) {
  return {
    id: 'profile-1',
    project_id: null,
    project: null,
    name: overrides.name ?? 'Direct profile',
    description: null,
    labels: {},
    proxy: null,
    bypass_rules: [],
    custom_ca: null,
    traffic_observation: {
      mode: 'metadata_only',
      sensitive_log_sink_ref: null,
      sensitive_log_sink_display_name: null,
    },
    state: 'ready',
    effective: {
      proxy_configured: false,
      proxy_auth_configured: false,
      bypass_rule_count: 0,
      custom_ca_configured: false,
      observation_mode: 'metadata_only',
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
    },
    diagnostics: {
      profile_id: 'profile-1',
      profile_name: overrides.name ?? 'Direct profile',
      profile_state: 'ready',
      health: 'ready',
      observation_mode: 'metadata_only',
      proof_level: 'configuration',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: false,
      proxy_auth_configured: false,
      bypass_rule_count: 0,
      custom_ca_configured: false,
      tls_interception_enabled: false,
      sensitive_log_sink_configured: false,
      proof: {
        profile_resolved: true,
        profile_ready: true,
        profile_reachability_collected: false,
        profile_reachability_healthy: false,
        profile_reachability_observed_at: null,
        profile_reachability_failure: null,
        proxy_launch_config_expected: false,
        bypass_rules_expected: 0,
        custom_ca_launch_config_expected: false,
        tls_interception_expected: false,
        sensitive_log_sink_declared: false,
        runtime_launch_observed: false,
        active_probe_collected: false,
        observed_public_ip: null,
        observed_tls_issuer: null,
        last_failure_reason: null,
      },
      warnings: [],
      observed_at: '2026-06-12T10:00:00.000Z',
    },
    created_at: '2026-06-12T09:00:00.000Z',
    updated_at: '2026-06-12T10:00:00.000Z',
  };
}
