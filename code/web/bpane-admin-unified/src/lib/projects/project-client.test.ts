import { describe, expect, it, vi } from 'vitest';

import {
  ProjectCatalogClient,
  ProjectCatalogError,
  toProjectListResponse,
} from './project-client';

describe('ProjectCatalogClient', () => {
  it('lists projects through the authenticated control API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response(JSON.stringify(projectListPayload()), { status: 200 }));
    const client = new ProjectCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listProjects();

    expect(response.projects).toHaveLength(1);
    expect(response.projects[0]?.name).toBe('Support');
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/projects'),
      expect.objectContaining({
        method: 'GET',
      }),
    );
    expect(headers.get('authorization')).toBe('Bearer token-1');
    expect(headers.get('accept')).toBe('application/json');
  });

  it('fetches, updates, and refreshes usage for one project', async () => {
    const fetchImpl = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = input.toString();
      if (init?.method === 'PUT') {
        return new Response(JSON.stringify({ ...projectPayload(), name: 'Support Ops' }), { status: 200 });
      }
      if (url.endsWith('/usage')) {
        return new Response(JSON.stringify(projectPayload().usage), { status: 200 });
      }
      return new Response(JSON.stringify(projectPayload()), { status: 200 });
    });
    const client = new ProjectCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const project = await client.getProject('11111111-1111-4111-8111-111111111111');
    const updated = await client.updateProject(project.id, {
      name: 'Support Ops',
      description: 'Updated',
      labels: { team: 'support' },
      quotas: project.quotas,
      policy: project.policy,
      state: 'active',
    });
    const usage = await client.getProjectUsage(project.id);

    expect(project.name).toBe('Support');
    expect(updated.name).toBe('Support Ops');
    expect(usage.project_id).toBe(project.id);
    expect(fetchImpl.mock.calls.map((call) => [call[0].toString(), call[1]?.method])).toEqual([
      ['http://browserpane.test/api/v1/projects/11111111-1111-4111-8111-111111111111', 'GET'],
      ['http://browserpane.test/api/v1/projects/11111111-1111-4111-8111-111111111111', 'PUT'],
      ['http://browserpane.test/api/v1/projects/11111111-1111-4111-8111-111111111111/usage', 'GET'],
    ]);
    expect(JSON.parse(fetchImpl.mock.calls[1]?.[1]?.body as string)).toMatchObject({
      name: 'Support Ops',
      labels: { team: 'support' },
      state: 'active',
    });
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
