import { describe, expect, it, vi } from 'vitest';

import {
  EgressProfileCatalogClient,
  EgressProfileCatalogError,
  toEgressProfileListResponse,
} from './egress-profile-client';

describe('EgressProfileCatalogClient', () => {
  it('loads egress profiles through the authenticated control API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify({ profiles: [egressProfilePayload()] }), { status: 200 }));
    const client = new EgressProfileCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listEgressProfiles();

    expect(response.profiles[0]).toMatchObject({
      id: 'profile-1',
      name: 'EU support egress',
      project_id: 'project-1',
      state: 'ready',
      effective: {
        proxy_configured: true,
        tls_interception_enabled: true,
      },
      diagnostics: {
        health: 'ready',
        proof_level: 'configuration',
      },
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/egress-profiles'),
      expect.objectContaining({
        method: 'GET',
      }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new EgressProfileCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listEgressProfiles()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('loads a single egress profile', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify(egressProfilePayload()), { status: 200 }));
    const client = new EgressProfileCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const profile = await client.getEgressProfile('profile-1');

    expect(profile.id).toBe('profile-1');
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/egress-profiles/profile-1'));
    expect(fetchImpl.mock.calls[0]?.[1]?.method).toBe('GET');
  });

  it('creates and updates egress profiles with JSON bodies', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify(egressProfilePayload()), { status: 200 }));
    const client = new EgressProfileCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });
    const request = {
      project_id: null,
      name: 'Direct profile',
      description: null,
      labels: {},
      proxy: null,
      bypass_rules: [],
      custom_ca: null,
      traffic_observation: {
        mode: 'metadata_only' as const,
        sensitive_log_sink_ref: null,
        sensitive_log_sink_display_name: null,
      },
      state: 'ready' as const,
    };

    await client.createEgressProfile(request);
    await client.updateEgressProfile('profile-1', request);

    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/egress-profiles'));
    expect(fetchImpl.mock.calls[0]?.[1]?.method).toBe('POST');
    expect(fetchImpl.mock.calls[0]?.[1]?.body).toBe(JSON.stringify(request));
    expect(fetchImpl.mock.calls[1]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/egress-profiles/profile-1'));
    expect(fetchImpl.mock.calls[1]?.[1]?.method).toBe('PUT');
    expect(fetchImpl.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(request));
  });

  it('loads project binding options from the project catalog', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify({
        projects: [{ id: 'project-1', name: 'Support', state: 'active', ignored: true }],
      }), { status: 200 }));
    const client = new EgressProfileCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listProjectOptions();

    expect(response.projects).toEqual([{ id: 'project-1', name: 'Support', state: 'active' }]);
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/projects'));
  });

  it('defaults optional effective and diagnostics payloads defensively', () => {
    const response = toEgressProfileListResponse({
      profiles: [{
        id: 'direct-profile',
        name: 'Direct profile',
        labels: {},
        bypass_rules: [],
        state: 'ready',
        created_at: '2026-06-12T10:00:00.000Z',
        updated_at: '2026-06-12T10:00:00.000Z',
      }],
    });

    expect(response.profiles[0]).toMatchObject({
      proxy: null,
      custom_ca: null,
      traffic_observation: { mode: 'metadata_only' },
      effective: {
        proxy_configured: false,
        tls_interception_enabled: false,
      },
      diagnostics: {
        health: 'unknown',
        proof_level: 'none',
      },
    });
  });

  it('rejects invalid list payloads', () => {
    expect(() => toEgressProfileListResponse({ profiles: {} })).toThrow(EgressProfileCatalogError);
  });
});

function egressProfilePayload() {
  return {
    id: 'profile-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'EU Support', state: 'active' },
    name: 'EU support egress',
    description: 'Support outbound path',
    labels: { region: 'eu' },
    proxy: {
      url: 'http://proxy.example:3128',
      credential_binding_id: 'credential-1',
    },
    bypass_rules: ['localhost'],
    custom_ca: {
      certificate_ref: 'file:///workspace/dev/egress-ca.pem',
      display_name: 'Local CA',
    },
    traffic_observation: {
      mode: 'tls_intercept',
      sensitive_log_sink_ref: 'siem://browserpane/eu-support',
      sensitive_log_sink_display_name: 'EU support SIEM',
    },
    state: 'ready',
    effective: {
      proxy_configured: true,
      proxy_auth_configured: true,
      bypass_rule_count: 1,
      custom_ca_configured: true,
      observation_mode: 'tls_intercept',
      tls_interception_enabled: true,
      sensitive_log_sink_configured: true,
    },
    diagnostics: {
      profile_id: 'profile-1',
      profile_name: 'EU support egress',
      profile_state: 'ready',
      health: 'ready',
      observation_mode: 'tls_intercept',
      proof_level: 'configuration',
      proof: {
        profile_resolved: true,
        profile_ready: true,
      },
      warnings: [],
      observed_at: '2026-06-12T10:00:00.000Z',
    },
    created_at: '2026-06-12T09:00:00.000Z',
    updated_at: '2026-06-12T10:00:00.000Z',
  };
}
