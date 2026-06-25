import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import EgressProfileCreateRoute from './EgressProfileCreateRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/egress/new');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('EgressProfileCreateRoute', () => {
  it('loads project options and creates a profile', async () => {
    const navigateToProfile = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/projects')) {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/egress-profiles') && init?.method === 'POST') {
        return jsonResponse(egressProfilePayload({ name: 'Created profile' }), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(EgressProfileCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToProfile,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'egress-profile-edit-form')).toBeInstanceOf(HTMLElement);
    });
    await input(target, 'egress-profile-edit-name', 'Created profile');
    byTestId(target, 'egress-profile-edit-save').click();

    await vi.waitFor(() => {
      expect(navigateToProfile).toHaveBeenCalledWith(expect.objectContaining({ id: 'profile-1' }));
    });
    const createCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/egress-profiles') && call[1]?.method === 'POST');
    expect(createCall?.[1]?.body).toBe(JSON.stringify({
      project_id: null,
      name: 'Created profile',
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
    }));
    const headers = createCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
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
