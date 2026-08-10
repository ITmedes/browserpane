import { describe, expect, it } from 'vitest';
import { projectResourceFixture } from '$lib/test-utils/project-fixture';

import {
  defaultSessionMode,
  initialWorkflowInputText,
  inputParametersFromSchema,
  validateWorkflowRunLaunch,
  workflowRunRequestPreview,
} from './workflow-run-launcher-view-model';
import type { WorkflowDefinitionVersionResource } from './workflow-types';

describe('workflow-run-launcher-view-model', () => {
  it('extracts flat input parameters and seeds defaults from schema', () => {
    const parameters = inputParametersFromSchema(schema());

    expect(parameters.map((parameter) => [parameter.name, parameter.kind, parameter.required])).toEqual([
      ['target_url', 'string', true],
      ['max_steps', 'number', false],
      ['debug', 'boolean', false],
    ]);
    expect(JSON.parse(initialWorkflowInputText(schema()))).toEqual({
      target_url: 'https://browserpane.io/',
      max_steps: 12,
      debug: false,
    });
  });

  it('builds create-session and existing-session run requests', () => {
    const createSession = validateWorkflowRunLaunch({
      workflowId: 'workflow-1',
      version: 'v1',
      inputSchema: schema(),
      inputText: JSON.stringify({ target_url: 'https://browserpane.io/' }),
      sessionMode: 'create_session',
      existingSessionId: '',
      projectId: 'project-1',
    });
    const existingSession = validateWorkflowRunLaunch({
      workflowId: 'workflow-1',
      version: 'v1',
      inputText: '{}',
      sessionMode: 'existing_session',
      existingSessionId: 'session-1',
      projectId: '',
    });

    expect(createSession).toMatchObject({
      ok: true,
      request: {
        project_id: 'project-1',
        session: { create_session: { project_id: 'project-1' } },
      },
    });
    expect(existingSession).toMatchObject({
      ok: true,
      request: { session: { existing_session_id: 'session-1' } },
    });
  });

  it('validates JSON, required input, and existing-session id', () => {
    expect(validateWorkflowRunLaunch({
      workflowId: 'workflow-1',
      version: 'v1',
      inputText: '{',
      sessionMode: 'create_session',
      existingSessionId: '',
      projectId: '',
    })).toMatchObject({ ok: false, message: expect.stringContaining('Input JSON is invalid') });
    expect(validateWorkflowRunLaunch({
      workflowId: 'workflow-1',
      version: 'v1',
      inputSchema: schema(),
      inputText: '{}',
      sessionMode: 'create_session',
      existingSessionId: '',
      projectId: '',
    })).toMatchObject({ ok: false, message: expect.stringContaining('target_url') });
    expect(validateWorkflowRunLaunch({
      workflowId: 'workflow-1',
      version: 'v1',
      inputText: '{}',
      sessionMode: 'existing_session',
      existingSessionId: '',
      projectId: '',
    })).toMatchObject({ ok: false, message: expect.stringContaining('Existing session id') });
  });

  it('uses version default sessions only when the version defines one', () => {
    expect(defaultSessionMode(version({ default_session: { labels: { origin: 'workflow' } } }))).toBe('version_default');
    expect(defaultSessionMode(version({ default_session: null }))).toBe('create_session');
    expect(workflowRunRequestPreview({
      workflowId: 'workflow-1',
      version: 'v1',
      inputText: '{}',
      sessionMode: 'version_default',
      existingSessionId: '',
      projectId: '',
    })).toContain('"workflow_id": "workflow-1"');
  });

  it('rejects unavailable and archived project selections when a catalog is loaded', () => {
    const base = {
      workflowId: 'workflow-1',
      version: 'v1',
      inputText: '{}',
      sessionMode: 'create_session' as const,
      existingSessionId: '',
      projectOptions: [projectResourceFixture({
        id: 'project-archived',
        name: 'Archived support',
        state: 'archived',
      })],
    };

    expect(validateWorkflowRunLaunch({ ...base, projectId: 'project-missing' }))
      .toMatchObject({ ok: false, message: 'Selected project is not available.' });
    expect(validateWorkflowRunLaunch({ ...base, projectId: 'project-archived' }))
      .toMatchObject({ ok: false, message: expect.stringContaining('archived') });
  });
});

function schema() {
  return {
    type: 'object',
    required: ['target_url'],
    properties: {
      target_url: {
        type: 'string',
        description: 'URL to open',
        default: 'https://browserpane.io/',
      },
      max_steps: {
        type: 'number',
        default: 12,
      },
      debug: {
        type: 'boolean',
        default: false,
      },
    },
  };
}

function version(overrides: Partial<WorkflowDefinitionVersionResource> = {}): WorkflowDefinitionVersionResource {
  return {
    id: 'version-1',
    workflow_definition_id: 'workflow-1',
    version: 'v1',
    executor: 'playwright',
    entrypoint: 'run.mjs',
    source: null,
    input_schema: schema(),
    output_schema: null,
    default_session: null,
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
    created_at: '2026-06-21T09:30:00.000Z',
    ...overrides,
  };
}
