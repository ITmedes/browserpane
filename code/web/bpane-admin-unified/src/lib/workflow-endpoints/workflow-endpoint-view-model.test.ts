import { describe, expect, it } from 'vitest';
import {
  buildWorkflowEndpointInvocationExample,
  createWorkflowEndpointDraft,
  createWorkflowEndpointGrantDraft,
  validateWorkflowEndpointDraft,
  validateWorkflowEndpointGrantDraft,
} from './workflow-endpoint-view-model';
import type { WorkflowEndpointResource } from './workflow-endpoint-types';

describe('workflow endpoint view model', () => {
  it('validates the bounded endpoint contract before an owner request', () => {
    const draft = createWorkflowEndpointDraft();
    Object.assign(draft, {
      endpointKey: 'retrieve-supplier-report',
      purpose: 'Retrieve a supplier report.',
      workflowDefinitionId: 'workflow-1',
      workflowDefinitionVersionId: 'version-1',
      workflowVersion: '1.0.0',
      labelsText: 'pilot=supplier',
    });
    expect(validateWorkflowEndpointDraft(draft)).toMatchObject({
      valid: true,
      request: {
        endpoint_key: 'retrieve-supplier-report',
        workflow_definition_version_id: 'version-1',
        execution_timeout_seconds: 900,
        artifact_behavior: { mode: 'authorized_references', retention_seconds: 86_400 },
        labels: { pilot: 'supplier' },
      },
    });

    Object.assign(draft, {
      endpointKey: '-BAD-',
      inputSchemaText: '{',
      executionTimeoutSeconds: '0',
      inlineResultMaxBytes: 'unbounded',
    });
    expect(validateWorkflowEndpointDraft(draft)).toMatchObject({
      valid: false,
      fieldErrors: {
        endpointKey: expect.any(Array),
        inputSchemaText: expect.any(Array),
        executionTimeoutSeconds: expect.any(Array),
        inlineResultMaxBytes: expect.any(Array),
      },
    });
  });

  it('requires a registered caller and at least one narrow operation', () => {
    const draft = createWorkflowEndpointGrantDraft();
    draft.operations = [];
    expect(validateWorkflowEndpointGrantDraft(draft)).toMatchObject({
      valid: false,
      fieldErrors: { servicePrincipalId: expect.any(Array), operations: expect.any(Array) },
    });
    draft.servicePrincipalId = 'principal-1';
    draft.operations = ['invoke', 'read', 'invoke'];
    expect(validateWorkflowEndpointGrantDraft(draft).request).toEqual({
      service_principal_id: 'principal-1',
      operations: ['invoke', 'read'],
    });
  });

  it('builds a secret-free polling invocation example from the published input schema', () => {
    const example = buildWorkflowEndpointInvocationExample(endpoint());
    expect(example).toContain('/projects/project-1/workflow-endpoints/report/invocations');
    expect(example).toContain('Idempotency-Key');
    expect(example).toContain('"reporting_period": "2026-Q3"');
    expect(example).toContain('$ACCESS_TOKEN');
    expect(example).not.toContain('client_secret');
  });
});

function endpoint(): WorkflowEndpointResource {
  return {
    id: 'endpoint-1',
    project_id: 'project-1',
    endpoint_key: 'report',
    purpose: 'Retrieve a report.',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'version-1',
    workflow_version: '1.0.0',
    input_schema: {
      type: 'object',
      properties: { reporting_period: { type: 'string', example: '2026-Q3' } },
    },
    output_schema: { type: 'object' },
    execution_timeout_seconds: 900,
    inline_result_max_bytes: 65_536,
    artifact_behavior: { mode: 'authorized_references', retention_seconds: 86_400 },
    supported_controls: ['poll', 'cancel'],
    labels: {},
    state: 'draft',
    grants_path: '/grants',
    invocations_path: '/invocations',
    created_at: '2026-08-21T08:00:00.000Z',
    updated_at: '2026-08-21T08:00:00.000Z',
  };
}
