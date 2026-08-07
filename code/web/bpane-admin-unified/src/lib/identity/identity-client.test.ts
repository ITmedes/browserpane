import { describe, expect, it, vi } from 'vitest';

import {
  IdentityCatalogClient,
  IdentityCatalogError,
  toIdentityAccessReviewResponse,
} from './identity-client';

describe('IdentityCatalogClient', () => {
  it('loads a sanitized access review through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify(accessReviewPayload()), { status: 200 }));
    const client = new IdentityCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const review = await client.getAccessReview();

    expect(review.principal).toEqual({
      subject: 'user-1',
      issuer: 'https://identity.example/realms/pane',
      display_name: 'Demo Operator',
      client_id: null,
      principal_type: 'user',
    });
    expect(review.projects[0]?.name).toBe('Operations');
    expect(review.service_principals[0]).toMatchObject({
      id: 'principal-1',
      delegated_session_count: 2,
      active_delegated_session_count: 1,
    });
    expect(review.identity_mappings[0]).toMatchObject({
      id: 'mapping-1',
      effective_for_principal: true,
    });
    expect(review.delegated_principals[0]?.registered).toBe(true);
    expect(review.unmapped_principal_signals[0]?.reason).toBe('group_without_project_mapping');
    expect(review.principal).not.toHaveProperty('raw_token');
    expect(review.service_principals[0]).not.toHaveProperty('client_secret');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/identity/access-review'),
      expect.objectContaining({ method: 'GET' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer owner-token');
  });

  it('loads the current principal independently', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify(accessReviewPayload().principal), { status: 200 }));
    const client = new IdentityCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const principal = await client.getCurrentPrincipal();

    expect(principal.subject).toBe('user-1');
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/identity/me'));
  });

  it('lists, fetches, creates, and updates service principals', async () => {
    const payload = servicePrincipalPayload();
    let callCount = 0;
    const fetchImpl = vi.fn<typeof fetch>(async (input): Promise<Response> => {
      callCount += 1;
      const url = requestUrl(input);
      return new Response(JSON.stringify(url.pathname === '/api/v1/service-principals'
        && callCount === 1
        ? { service_principals: [payload] }
        : payload), { status: 200 });
    });
    const client = new IdentityCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });
    const request = {
      name: 'Workflow bridge',
      client_id: 'bpane-workflow-bridge',
      issuer: 'https://identity.example/realms/pane',
      labels: { purpose: 'workflow' },
      scopes: ['session:delegate'],
      allowed_project_ids: ['project-1'],
      state: 'active' as const,
    };

    expect((await client.listServicePrincipals()).service_principals).toHaveLength(1);
    await client.getServicePrincipal('principal/with space');
    await client.createServicePrincipal(request);
    await client.updateServicePrincipal('principal-1', { ...request, state: 'disabled' });

    expect(fetchImpl.mock.calls.map((call) => [requestUrl(call[0]).pathname, call[1]?.method])).toEqual([
      ['/api/v1/service-principals', 'GET'],
      ['/api/v1/service-principals/principal%2Fwith%20space', 'GET'],
      ['/api/v1/service-principals', 'POST'],
      ['/api/v1/service-principals/principal-1', 'PUT'],
    ]);
    expect(fetchImpl.mock.calls[2]?.[1]?.body).toBe(JSON.stringify(request));
  });

  it('lists, fetches, creates, and updates identity mappings', async () => {
    const payload = identityMappingPayload();
    let callCount = 0;
    const fetchImpl = vi.fn<typeof fetch>(async (input): Promise<Response> => {
      callCount += 1;
      const url = requestUrl(input);
      return new Response(JSON.stringify(url.pathname === '/api/v1/identity-mappings'
        && callCount === 1
        ? { identity_mappings: [payload] }
        : payload), { status: 200 });
    });
    const client = new IdentityCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });
    const request = {
      name: 'Operations group',
      kind: 'group' as const,
      issuer: 'https://identity.example/realms/pane',
      external_id: 'operations',
      project_id: 'project-1',
      labels: {},
      scopes: ['project:read'],
      state: 'active' as const,
    };

    expect((await client.listIdentityMappings()).identity_mappings).toHaveLength(1);
    await client.getIdentityMapping('mapping/with space');
    await client.createIdentityMapping(request);
    await client.updateIdentityMapping('mapping-1', { ...request, state: 'disabled' });

    expect(fetchImpl.mock.calls.map((call) => [requestUrl(call[0]).pathname, call[1]?.method])).toEqual([
      ['/api/v1/identity-mappings', 'GET'],
      ['/api/v1/identity-mappings/mapping%2Fwith%20space', 'GET'],
      ['/api/v1/identity-mappings', 'POST'],
      ['/api/v1/identity-mappings/mapping-1', 'PUT'],
    ]);
    expect(fetchImpl.mock.calls[2]?.[1]?.body).toBe(JSON.stringify(request));
  });

  it('delegates authentication failures to global recovery', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new IdentityCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response(JSON.stringify({ error: 'expired' }), { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.getAccessReview()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

describe('identity response mapping', () => {
  it('rejects malformed collections, counts, enums, and secret-shaped substitutes', () => {
    expect(() => toIdentityAccessReviewResponse({
      ...accessReviewPayload(),
      delegated_principals: {},
    })).toThrow(IdentityCatalogError);
    expect(() => toIdentityAccessReviewResponse({
      ...accessReviewPayload(),
      resource_counts: { ...resourceCounts(), sessions: -1 },
    })).toThrow('must be a non-negative integer');
    expect(() => toIdentityAccessReviewResponse({
      ...accessReviewPayload(),
      principal: { ...accessReviewPayload().principal, principal_type: 'root' },
    })).toThrow('unsupported value');
    expect(() => toIdentityAccessReviewResponse({
      ...accessReviewPayload(),
      principal: {
        raw_token: 'secret-token',
        issuer: 'https://identity.example/realms/pane',
        display_name: 'Unsafe fixture',
        client_id: null,
        principal_type: 'user',
      },
    })).toThrow('subject must be a non-empty string');
  });
});

export function accessReviewPayload() {
  return {
    principal: {
      subject: 'user-1',
      issuer: 'https://identity.example/realms/pane',
      display_name: 'Demo Operator',
      client_id: null,
      principal_type: 'user',
      raw_token: 'ignored-token',
    },
    generated_at: '2026-08-07T10:00:00.000Z',
    projects: [projectPayload()],
    resource_counts: resourceCounts(),
    identity_mappings: [{ ...identityMappingPayload(), effective_for_principal: true }],
    unmapped_principal_signals: [{
      kind: 'group',
      issuer: 'https://identity.example/realms/pane',
      external_id: 'support',
      claim_name: null,
      display_name: 'Support',
      reason: 'group_without_project_mapping',
    }],
    service_principals: [{
      ...servicePrincipalPayload(),
      client_secret: 'ignored-secret',
      delegated_session_count: 2,
      active_delegated_session_count: 1,
      delegated_session_ids: ['session-1', 'session-2'],
    }],
    delegated_principals: [{
      client_id: 'bpane-mcp-bridge',
      issuer: 'https://identity.example/realms/pane',
      display_name: 'MCP bridge',
      registered: true,
      registered_service_principal_id: 'principal-1',
      state: 'active',
      session_count: 2,
      active_session_count: 1,
      session_ids: ['session-1', 'session-2'],
    }],
  };
}

function servicePrincipalPayload() {
  return {
    id: 'principal-1',
    name: 'MCP bridge',
    description: 'External automation identity',
    client_id: 'bpane-mcp-bridge',
    issuer: 'https://identity.example/realms/pane',
    labels: { purpose: 'automation' },
    scopes: ['session:delegate'],
    allowed_project_ids: ['project-1'],
    state: 'active',
    last_seen_at: '2026-08-07T09:00:00.000Z',
    last_delegated_at: '2026-08-07T09:30:00.000Z',
    created_at: '2026-08-01T10:00:00.000Z',
    updated_at: '2026-08-07T09:30:00.000Z',
  };
}

function identityMappingPayload() {
  return {
    id: 'mapping-1',
    name: 'Demo operator access',
    description: 'Maps the demo operator to Operations',
    kind: 'user',
    issuer: 'https://identity.example/realms/pane',
    external_id: 'user-1',
    claim_name: null,
    service_principal_id: null,
    project_id: 'project-1',
    labels: { source: 'admin' },
    scopes: ['project:read'],
    state: 'active',
    last_seen_at: '2026-08-07T09:00:00.000Z',
    created_at: '2026-08-01T10:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}

function resourceCounts() {
  return {
    projects: 1,
    service_principals: 1,
    identity_mappings: 1,
    sessions: 2,
    active_sessions: 1,
    session_templates: 1,
    browser_contexts: 1,
    egress_profiles: 1,
    credential_bindings: 1,
    file_workspaces: 1,
    workflow_definitions: 1,
    workflow_runs: 3,
    active_workflow_runs: 1,
    automation_tasks: 4,
    active_automation_tasks: 1,
    extension_definitions: 1,
    delegated_principals: 1,
  };
}

function projectPayload() {
  return {
    id: 'project-1',
    name: 'Operations',
    description: 'Operations automation',
    labels: {},
    quotas: {},
    policy: {},
    state: 'active',
    usage: {
      project_id: 'project-1',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 2,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 1,
      max_active_workflow_runs: null,
      runtime_usage_ms: 1200,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 100,
      egress_tx_bytes: 50,
      egress_total_bytes: 150,
      max_egress_total_bytes: null,
      retained_storage_bytes: 1024,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-08-07T10:00:00.000Z',
    },
    created_at: '2026-08-01T10:00:00.000Z',
    updated_at: '2026-08-07T10:00:00.000Z',
  };
}

function requestUrl(input: RequestInfo | URL): URL {
  if (input instanceof URL) {
    return input;
  }
  if (typeof input === 'string') {
    return new URL(input);
  }
  return new URL(input.url);
}
