import type { ApiContractEvidence, ApiExample, ApiOperation } from '$lib/api-companion/api-contract-types';

export function apiContractEvidenceFixture(): ApiContractEvidence {
  const operations: readonly ApiOperation[] = [
    operation('createProject', 'POST', '/api/v1/projects', 'Projects', 'owner-bearer', 'ui-primary', ['201', '401']),
    operation('createSession', 'POST', '/api/v1/sessions', 'Sessions', 'owner-bearer', 'ui-primary', ['201', '401']),
    operation('issueSessionAccessToken', 'POST', '/api/v1/sessions/{session_id}/access-tokens', 'Session Runtime', 'owner-bearer', 'ui-evidence', ['200', '401']),
    operation('createWorkflowRun', 'POST', '/api/v1/workflow-runs', 'Workflows', 'owner-bearer', 'ui-primary', ['201', '401']),
    operation('listWorkflowDefinitions', 'GET', '/api/v1/workflows', 'Workflows', 'owner-bearer', 'ui-primary', ['200', '401']),
    operation('createFileWorkspace', 'POST', '/api/v1/file-workspaces', 'File Workspaces', 'owner-bearer', 'ui-primary', ['201', '401']),
    operation('listFileWorkspaces', 'GET', '/api/v1/file-workspaces', 'File Workspaces', 'owner-bearer', 'ui-primary', ['200', '401']),
    operation('cancelAutomationTask', 'POST', '/api/v1/automation-tasks/{task_id}/cancel', 'Automation Tasks', 'owner-bearer', 'api-companion', ['200', '401']),
    operation('appendWorkflowRunLog', 'POST', '/api/v1/workflow-runs/{run_id}/logs', 'Workflows', 'session-automation', 'internal-worker', ['200', '401']),
    operation('openAdminEvents', 'GET', '/api/v1/admin/events', 'Admin Events', 'unauthenticated', 'ui-evidence', ['101']),
  ];
  return {
    operations,
    classifications: {
      'ui-primary': ['createProject', 'createSession', 'createWorkflowRun', 'listWorkflowDefinitions', 'createFileWorkspace', 'listFileWorkspaces'],
      'ui-evidence': ['issueSessionAccessToken', 'openAdminEvents'],
      'api-companion': ['cancelAutomationTask'],
      'internal-worker': ['appendWorkflowRunLog'],
    },
    examples: [
      example('companion-project-create', 'createProject', '/api/v1/projects', { name: 'Pilot' }),
      example('companion-session-create', 'createSession', '/api/v1/sessions', { project_id: '11111111-1111-4111-8111-111111111111' }),
      example('companion-session-connect-ticket', 'issueSessionAccessToken', '/api/v1/sessions/22222222-2222-4222-8222-222222222222/access-tokens'),
      example('companion-workflow-run-create', 'createWorkflowRun', '/api/v1/workflow-runs', { workflow_id: '33333333-3333-4333-8333-333333333333' }),
      getExample('workflow-definitions-empty-list', 'listWorkflowDefinitions', '/api/v1/workflows'),
      example('companion-file-workspace-create', 'createFileWorkspace', '/api/v1/file-workspaces', { project_id: '11111111-1111-4111-8111-111111111111' }),
      getExample('file-workspaces-empty-list', 'listFileWorkspaces', '/api/v1/file-workspaces'),
    ],
    compatibilitySurfaces: [
      {
        id: 'legacy-session-status',
        family: 'Gateway legacy',
        methods: ['GET'],
        path: '/api/session/status',
        auth: 'owner-bearer',
        stability: 'legacy',
        purpose: 'Preserves the pre-v1 session status contract.',
      },
      {
        id: 'mcp-health',
        family: 'MCP bridge',
        methods: ['GET'],
        path: '/health',
        auth: 'deployment-internal',
        stability: 'compatibility',
        purpose: 'Reports bridge-local health.',
      },
    ],
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
