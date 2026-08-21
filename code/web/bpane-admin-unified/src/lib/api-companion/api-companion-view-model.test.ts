import { describe, expect, it } from 'vitest';
import type { ApiContractEvidence, ApiExample, ApiOperation } from './api-contract-types';
import {
  authDefinition,
  authSummaries,
  buildApiTaskFlows,
  classificationDefinition,
  classificationSummaries,
  commandForExample,
  filterApiOperations,
  groupCompatibilitySurfaces,
  operationFamilies,
} from './api-companion-view-model';

const OPERATIONS: readonly ApiOperation[] = [
  operation('createProject', 'POST', '/api/v1/projects', 'Projects', 'owner-bearer', 'ui-primary', ['201', '401']),
  operation('createSession', 'POST', '/api/v1/sessions', 'Sessions', 'owner-bearer', 'ui-primary', ['201', '401']),
  operation('issueSessionAccessToken', 'POST', '/api/v1/sessions/{session_id}/access-tokens', 'Session Runtime', 'owner-bearer', 'ui-evidence', ['200', '401']),
  operation('createWorkflowRun', 'POST', '/api/v1/workflow-runs', 'Workflows', 'owner-bearer', 'ui-primary', ['201', '401']),
  operation('listWorkflowDefinitions', 'GET', '/api/v1/workflows', 'Workflows', 'owner-bearer', 'ui-primary', ['200', '401']),
  operation('createFileWorkspace', 'POST', '/api/v1/file-workspaces', 'File Workspaces', 'owner-bearer', 'ui-primary', ['201', '401']),
  operation('listFileWorkspaces', 'GET', '/api/v1/file-workspaces', 'File Workspaces', 'owner-bearer', 'ui-primary', ['200', '401']),
  operation('appendWorkflowRunLog', 'POST', '/api/v1/workflow-runs/{run_id}/logs', 'Workflows', 'session-automation', 'internal-worker', ['200']),
  operation('finalizeRecording', 'POST', '/api/v1/sessions/{session_id}/recordings/{recording_id}/finalize', 'Session Recordings', 'recording-worker', 'internal-worker', ['200']),
  operation('openAdminEvents', 'GET', '/api/v1/admin/events', 'Admin Events', 'unauthenticated', 'ui-evidence', ['101']),
  operation('invokeWorkflowEndpoint', 'POST', '/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations', 'Workflow Endpoints', 'machine-bearer', 'api-companion', ['202', '409']),
  operation('listWorkflowEndpoints', 'GET', '/api/v1/projects/{project_id}/workflow-endpoints', 'Workflow Endpoints', 'owner-bearer', 'ui-primary', ['200']),
];

const EXAMPLES: readonly ApiExample[] = [
  example('companion-project-create', 'createProject', '/api/v1/projects', { name: 'Pilot' }),
  example('companion-session-create', 'createSession', '/api/v1/sessions', {
    project_id: '11111111-1111-4111-8111-111111111111',
  }),
  example(
    'companion-session-connect-ticket',
    'issueSessionAccessToken',
    '/api/v1/sessions/22222222-2222-4222-8222-222222222222/access-tokens',
  ),
  example('companion-workflow-run-create', 'createWorkflowRun', '/api/v1/workflow-runs', {
    workflow_id: '33333333-3333-4333-8333-333333333333',
  }),
  getExample('workflow-definitions-empty-list', 'listWorkflowDefinitions', '/api/v1/workflows'),
  example('companion-file-workspace-create', 'createFileWorkspace', '/api/v1/file-workspaces', {
    project_id: '11111111-1111-4111-8111-111111111111',
  }),
  getExample('file-workspaces-empty-list', 'listFileWorkspaces', '/api/v1/file-workspaces'),
  getExample('workflow-endpoints-empty-list', 'listWorkflowEndpoints', '/api/v1/projects/11111111-1111-4111-8111-111111111111/workflow-endpoints'),
  {
    ...example(
      'workflow-endpoint-idempotency-conflict',
      'invokeWorkflowEndpoint',
      '/api/v1/projects/11111111-1111-4111-8111-111111111111/workflow-endpoints/report/invocations',
      { input: { reporting_period: '2026-Q3' } },
    ),
    request: {
      method: 'POST',
      path: '/api/v1/projects/11111111-1111-4111-8111-111111111111/workflow-endpoints/report/invocations',
      headers: { 'idempotency-key': 'process-123-activity-1' },
      body: { input: { reporting_period: '2026-Q3' } },
    },
  },
];

const EVIDENCE: ApiContractEvidence = {
  operations: OPERATIONS,
  classifications: {
    'ui-primary': ['createProject', 'createSession', 'createWorkflowRun', 'listWorkflowDefinitions', 'createFileWorkspace', 'listFileWorkspaces', 'listWorkflowEndpoints'],
    'ui-evidence': ['issueSessionAccessToken', 'openAdminEvents'],
    'api-companion': ['invokeWorkflowEndpoint'],
    'internal-worker': ['appendWorkflowRunLog', 'finalizeRecording'],
  },
  examples: EXAMPLES,
  compatibilitySurfaces: [
    {
      id: 'legacy-status',
      family: 'Gateway legacy',
      methods: ['GET'],
      path: '/api/session/status',
      auth: 'owner-bearer',
      stability: 'legacy',
      purpose: 'Legacy status.',
    },
    {
      id: 'mcp-health',
      family: 'MCP bridge',
      methods: ['GET'],
      path: '/health',
      auth: 'deployment-internal',
      stability: 'compatibility',
      purpose: 'Bridge health.',
    },
    {
      id: 'legacy-owner',
      family: 'Gateway legacy',
      methods: ['POST'],
      path: '/api/session/mcp-owner',
      auth: 'owner-bearer',
      stability: 'legacy',
      purpose: 'Legacy owner.',
    },
  ],
};

describe('API companion task flows', () => {
  it('builds all task groups from canonical examples', () => {
    const flows = buildApiTaskFlows(EVIDENCE);
    expect(flows.map((flow) => flow.id)).toEqual(['projects', 'sessions', 'workflows', 'workflow-endpoints', 'file-workspaces']);
    expect(flows[1]?.steps.map((step) => step.title)).toEqual(['Create session', 'Mint connect ticket']);
    expect(flows[0]?.steps[0]?.coverageHref).toBe('/admin-new/coverage?operation=createProject');
  });

  it('fails visibly when a required canonical example is missing', () => {
    expect(() => buildApiTaskFlows({ ...EVIDENCE, examples: EXAMPLES.slice(1) }))
      .toThrow('Required API companion example is missing: companion-project-create');
  });

  it('generates token-free owner commands and replaces fixture ids with variables', () => {
    const sessionCommand = commandForExample(OPERATIONS[1]!, EXAMPLES[1]!);
    expect(sessionCommand).toContain('Authorization: Bearer ${BPANE_OWNER_TOKEN}');
    expect(sessionCommand).toContain('"project_id": "${BPANE_PROJECT_ID}"');
    expect(sessionCommand).not.toContain('11111111-1111-4111-8111-111111111111');
    expect(sessionCommand).not.toMatch(/demo-demo|access_token|client_secret/);

    const ticketCommand = commandForExample(OPERATIONS[2]!, EXAMPLES[2]!);
    expect(ticketCommand).toContain('/sessions/${BPANE_SESSION_ID}/access-tokens');
    expect(ticketCommand).not.toContain('--data');
    expect(ticketCommand.trimEnd()).not.toMatch(/\\$/);
  });

  it('uses the narrow worker credential only for worker operations', () => {
    const workerExample: ApiExample = {
      name: 'worker-log',
      operationId: 'appendWorkflowRunLog',
      request: { method: 'POST', path: '/api/v1/workflow-runs/run-1/logs', body: { line: 'safe' } },
      response: { status: 200 },
    };
    const command = commandForExample(OPERATIONS.find((item) => item.operationId === 'appendWorkflowRunLog')!, workerExample);
    expect(command).toContain('${BPANE_SESSION_AUTOMATION_TOKEN}');
    expect(command).not.toContain('${BPANE_OWNER_TOKEN}');

    const recordingExample: ApiExample = {
      name: 'recording-finalize',
      operationId: 'finalizeRecording',
      request: {
        method: 'POST',
        path: '/api/v1/sessions/session-1/recordings/recording-1/finalize',
      },
      response: { status: 200 },
    };
    const recordingCommand = commandForExample(
      OPERATIONS.find((item) => item.operationId === 'finalizeRecording')!,
      recordingExample,
    );
    expect(recordingCommand).toContain('${BPANE_RECORDING_WORKER_TOKEN}');
    expect(recordingCommand).not.toContain('${BPANE_OWNER_TOKEN}');
  });
});

describe('API coverage projections', () => {
  it('derives stable families and exact summaries', () => {
    expect(operationFamilies(OPERATIONS)).toEqual([
      'Admin Events',
      'File Workspaces',
      'Projects',
      'Session Recordings',
      'Session Runtime',
      'Sessions',
      'Workflow Endpoints',
      'Workflows',
    ]);
    expect(classificationSummaries(OPERATIONS).map(({ id, count }) => [id, count])).toEqual([
      ['ui-primary', 7],
      ['ui-evidence', 2],
      ['api-companion', 1],
      ['internal-worker', 2],
    ]);
    expect(authSummaries(OPERATIONS).map(({ id, count }) => [id, count])).toEqual([
      ['owner-bearer', 8],
      ['session-automation', 1],
      ['recording-worker', 1],
      ['machine-bearer', 1],
      ['unauthenticated', 1],
    ]);
  });

  it('filters by search, classification, auth, and family with stable ordering', () => {
    expect(filterApiOperations(OPERATIONS, {
      query: 'workflow',
      classification: 'all',
      auth: 'all',
      family: 'all',
    }).map((item) => item.operationId)).toEqual([
      'listWorkflowEndpoints',
      'invokeWorkflowEndpoint',
      'createWorkflowRun',
      'appendWorkflowRunLog',
      'listWorkflowDefinitions',
    ]);
    expect(filterApiOperations(OPERATIONS, {
      query: '',
      classification: 'internal-worker',
      auth: 'session-automation',
      family: 'Workflows',
    }).map((item) => item.operationId)).toEqual(['appendWorkflowRunLog']);
  });

  it('groups compatibility surfaces separately and sorts entries', () => {
    expect(groupCompatibilitySurfaces(EVIDENCE.compatibilitySurfaces)).toEqual([
      {
        family: 'Gateway legacy',
        surfaces: [EVIDENCE.compatibilitySurfaces[2], EVIDENCE.compatibilitySurfaces[0]],
      },
      { family: 'MCP bridge', surfaces: [EVIDENCE.compatibilitySurfaces[1]] },
    ]);
  });

  it('resolves stable classification and auth definitions', () => {
    expect(classificationDefinition('internal-worker').label).toBe('Internal worker');
    expect(authDefinition('owner-bearer').label).toBe('Owner bearer');
    expect(authDefinition('recording-worker').label).toBe('Recording worker');
    expect(authDefinition('machine-bearer').label).toBe('Machine bearer');
  });

  it('generates a machine-token endpoint command with the canonical idempotency header', () => {
    const operation = OPERATIONS.find((item) => item.operationId === 'invokeWorkflowEndpoint')!;
    const example = EXAMPLES.find((item) => item.name === 'workflow-endpoint-idempotency-conflict')!;
    const command = commandForExample(operation, example);
    expect(command).toContain('Authorization: Bearer ${BPANE_MACHINE_TOKEN}');
    expect(command).toContain('idempotency-key: process-123-activity-1');
    expect(command).not.toContain('${BPANE_OWNER_TOKEN}');
  });
});

function operation(
  operationId: string,
  method: ApiOperation['method'],
  path: string,
  tag: string,
  auth: ApiOperation['auth'],
  classification: ApiOperation['classification'],
  responses: readonly string[],
): ApiOperation {
  return { operationId, method, path, tags: [tag], auth, classification, responses };
}

function example(name: string, operationId: string, path: string, body?: unknown): ApiExample {
  return {
    name,
    operationId,
    request: { method: 'POST', path, ...(body === undefined ? {} : { body }) },
    response: { status: 401, body: { error: 'missing bearer token' } },
  };
}

function getExample(name: string, operationId: string, path: string): ApiExample {
  return {
    name,
    operationId,
    request: { method: 'GET', path },
    response: { status: 200 },
  };
}
