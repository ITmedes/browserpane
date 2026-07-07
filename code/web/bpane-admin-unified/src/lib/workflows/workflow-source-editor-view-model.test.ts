import { describe, expect, it } from 'vitest';

import {
  createWorkflowSourceEditorDraft,
  validateWorkflowSourceEditorDraft,
  workflowSourceEditorRequestKey,
} from './workflow-source-editor-view-model';
import type { WorkflowDefinitionVersionResource } from './workflow-types';

describe('workflow source editor view model', () => {
  it('creates the next immutable version from a selected version', () => {
    const draft = createWorkflowSourceEditorDraft([version('v1')], version('v1'));

    expect(draft).toMatchObject({
      version: 'v2',
      executor: 'playwright',
      repositoryUrl: '/workspace',
      ref: 'HEAD',
      rootPath: 'dev',
      entrypoint: 'dev/workflows/demo/run.mjs',
    });
  });

  it('builds validation and create-version requests', () => {
    const validation = validateWorkflowSourceEditorDraft(
      {
        version: 'v2',
        executor: 'playwright',
        repositoryUrl: 'https://example.test/repo.git',
        ref: 'main',
        rootPath: 'workflows',
        entrypoint: 'workflows/run.ts',
      },
      [version('v1')],
    );

    expect(validation.valid).toBe(true);
    expect(validation.sourceRequest).toMatchObject({
      entrypoint: 'workflows/run.ts',
      source: {
        kind: 'git',
        repository_url: 'https://example.test/repo.git',
        ref: 'main',
        root_path: 'workflows',
      },
    });
    expect(validation.request?.version).toBe('v2');
    expect(workflowSourceEditorRequestKey(validation.sourceRequest)).toContain('workflows/run.ts');
  });

  it('rejects duplicate versions and incomplete source metadata', () => {
    const validation = validateWorkflowSourceEditorDraft(
      {
        version: 'v1',
        executor: '',
        repositoryUrl: '',
        ref: '',
        rootPath: '',
        entrypoint: '',
      },
      [version('v1')],
    );

    expect(validation.valid).toBe(false);
    expect(validation.fieldErrors.version?.[0]).toContain('already exists');
    expect(validation.fieldErrors.repositoryUrl?.[0]).toContain('required');
    expect(validation.request).toBeNull();
  });
});

function version(value: string): WorkflowDefinitionVersionResource {
  return {
    id: `version-${value}`,
    workflow_definition_id: 'workflow-1',
    version: value,
    executor: 'playwright',
    entrypoint: 'dev/workflows/demo/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    allowed_credential_binding_ids: [],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: [],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}
