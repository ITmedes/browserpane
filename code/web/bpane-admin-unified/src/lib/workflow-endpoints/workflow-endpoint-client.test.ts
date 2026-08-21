import { describe, expect, it, vi } from 'vitest';
import { WorkflowEndpointCatalogClient, WorkflowEndpointCatalogError } from './workflow-endpoint-client';
import type { UpsertWorkflowEndpointRequest } from './workflow-endpoint-types';

describe('WorkflowEndpointCatalogClient', () => {
  it('manages project endpoints, lifecycle, and narrow caller grants', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = new URL(String(input));
      if (init?.method === 'DELETE') return new Response(null, { status: 204 });
      if (url.pathname.endsWith('/grants') && init?.method === 'GET') {
        return jsonResponse({ grants: [grantPayload()] });
      }
      if (url.pathname.endsWith('/grants')) return jsonResponse(grantPayload());
      if (url.pathname.endsWith('/workflow-endpoints') && init?.method === 'GET') {
        return jsonResponse({ workflow_endpoints: [endpointPayload()] });
      }
      return jsonResponse(endpointPayload(), init?.method === 'POST' ? 201 : 200);
    });
    const client = new WorkflowEndpointCatalogClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    await client.listEndpoints('project/one');
    await client.createEndpoint('project/one', endpointRequest());
    await client.getEndpoint('project/one', 'supplier/report');
    await client.updateEndpoint('project/one', 'supplier/report', endpointRequest());
    await client.activateEndpoint('project/one', 'supplier/report');
    await client.disableEndpoint('project/one', 'supplier/report');
    await client.listGrants('project/one', 'supplier/report');
    await client.upsertGrant('project/one', 'supplier/report', {
      service_principal_id: 'principal-1',
      operations: ['invoke', 'read'],
    });
    await client.revokeGrant('project/one', 'supplier/report', 'grant/one');

    expect(fetchImpl).toHaveBeenCalledTimes(9);
    expect(String(fetchImpl.mock.calls[0]?.[0])).toContain('/projects/project%2Fone/workflow-endpoints');
    expect(String(fetchImpl.mock.calls[2]?.[0])).toContain('/supplier%2Freport');
    expect(new Headers(fetchImpl.mock.calls[0]?.[1]?.headers).get('authorization')).toBe('Bearer owner-token');
    expect(fetchImpl.mock.calls[7]?.[1]?.body).toContain('principal-1');
  });

  it('rejects malformed resources and retains RFC problem codes for denied actions', async () => {
    const malformed = new WorkflowEndpointCatalogClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl: async () => jsonResponse({ workflow_endpoints: [{ ...endpointPayload(), state: 'unknown' }] }),
    });
    await expect(malformed.listEndpoints('project-1')).rejects.toBeInstanceOf(WorkflowEndpointCatalogError);

    const denied = new WorkflowEndpointCatalogClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'owner-token',
      fetchImpl: async () => jsonResponse({
        type: 'https://browserpane.dev/problems/service-principal-project-mismatch',
        title: 'Service principal is not allowed for this project',
        status: 400,
        detail: 'The registered caller belongs to another project.',
        code: 'service_principal_project_mismatch',
      }, 400),
    });
    await expect(denied.activateEndpoint('project-1', 'report')).rejects.toMatchObject({
      status: 400,
      apiCode: 'service_principal_project_mismatch',
      apiMessage: 'The registered caller belongs to another project.',
    });
  });
});

function endpointRequest(): UpsertWorkflowEndpointRequest {
  return {
    endpoint_key: 'supplier-report',
    purpose: 'Retrieve a supplier report.',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'version-1',
    workflow_version: '1.0.0',
    input_schema: { type: 'object' },
    output_schema: { type: 'object' },
    execution_timeout_seconds: 900,
    inline_result_max_bytes: 65_536,
    artifact_behavior: { mode: 'authorized_references', retention_seconds: 86_400 },
    labels: {},
  };
}

function endpointPayload() {
  return {
    id: 'endpoint-1',
    project_id: 'project-1',
    ...endpointRequest(),
    supported_controls: ['poll', 'cancel'],
    state: 'draft',
    grants_path: '/api/v1/projects/project-1/workflow-endpoints/supplier-report/grants',
    invocations_path: '/api/v1/projects/project-1/workflow-endpoints/supplier-report/invocations',
    created_at: '2026-08-21T08:00:00.000Z',
    updated_at: '2026-08-21T08:00:00.000Z',
  };
}

function grantPayload() {
  return {
    id: 'grant-1',
    endpoint_id: 'endpoint-1',
    project_id: 'project-1',
    service_principal_id: 'principal-1',
    operations: ['invoke', 'read'],
    created_at: '2026-08-21T08:00:00.000Z',
    updated_at: '2026-08-21T08:00:00.000Z',
  };
}

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), { status, headers: { 'content-type': 'application/json' } });
}
