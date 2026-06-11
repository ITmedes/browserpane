import { describe, expect, it, vi } from 'vitest';

import {
  loadStoredAdminAccessToken,
  ProjectCatalogClient,
  ProjectCatalogError,
  toProjectListResponse,
} from './project-client';

const TOKEN = JSON.stringify({
  access_token: 'admin-token',
  expiresAtMs: 2_000_000,
});

describe('loadStoredAdminAccessToken', () => {
  it('returns a non-expired access token from the existing admin token store', () => {
    const storage = { getItem: vi.fn(() => TOKEN) };

    expect(loadStoredAdminAccessToken(storage, 1_000_000)).toBe('admin-token');
  });

  it('returns null for expired or malformed token payloads', () => {
    expect(
      loadStoredAdminAccessToken(
        { getItem: () => JSON.stringify({ access_token: 'old', expiresAtMs: 100 }) },
        100,
      ),
    ).toBeNull();
    expect(loadStoredAdminAccessToken({ getItem: () => 'not-json' }, 100)).toBeNull();
    expect(loadStoredAdminAccessToken({ getItem: () => null }, 100)).toBeNull();
  });
});

describe('ProjectCatalogClient', () => {
  it('lists projects through the authenticated control API', async () => {
    const fetchImpl = vi.fn(async () => new Response(JSON.stringify(projectListPayload()), { status: 200 }));
    const client = new ProjectCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listProjects();

    expect(response.projects).toHaveLength(1);
    expect(response.projects[0]?.name).toBe('Support');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/projects'),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          authorization: 'Bearer token-1',
        }),
      }),
    );
  });

  it('reports authentication failures before throwing the HTTP error', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new ProjectCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listProjects()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('rejects missing tokens without calling the API', async () => {
    const fetchImpl = vi.fn();
    const client = new ProjectCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => null,
      fetchImpl,
    });

    await expect(client.listProjects()).rejects.toMatchObject({ code: 'missing_token' });
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

describe('toProjectListResponse', () => {
  it('maps project payload defaults and rejects unsupported enum values', () => {
    const mapped = toProjectListResponse(projectListPayload());

    expect(mapped.projects[0]).toMatchObject({
      id: '11111111-1111-4111-8111-111111111111',
      state: 'active',
      policy: {
        allow_browser_uploads: true,
        usage_budget_enforcement: 'warning_only',
      },
    });

    expect(() =>
      toProjectListResponse({
        projects: [{ ...projectPayload(), state: 'paused' }],
      }),
    ).toThrow(ProjectCatalogError);
  });
});

function projectListPayload() {
  return { projects: [projectPayload()] };
}

function projectPayload() {
  return {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'Support',
    description: 'Support browser work',
    labels: { team: 'support' },
    quotas: {},
    policy: {},
    state: 'active',
    usage: {
      project_id: '11111111-1111-4111-8111-111111111111',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 4,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 60_000,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 512,
      egress_tx_bytes: 512,
      egress_total_bytes: 1024,
      max_egress_total_bytes: null,
      retained_storage_bytes: 2048,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
