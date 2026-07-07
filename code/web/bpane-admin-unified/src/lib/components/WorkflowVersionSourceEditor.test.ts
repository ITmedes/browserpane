import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import type {
  CreateWorkflowDefinitionVersionRequest,
  ValidateWorkflowDefinitionSourceRequest,
  WorkflowDefinitionVersionResource,
} from '$lib/workflows/workflow-types';
import WorkflowVersionSourceEditor from './WorkflowVersionSourceEditor.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowVersionSourceEditor', () => {
  it('validates a workflow source and creates an immutable version', async () => {
    const onValidateSource = vi.fn(async (_request: ValidateWorkflowDefinitionSourceRequest) => sourceValidation());
    const onCreateVersion = vi.fn(async (_request: CreateWorkflowDefinitionVersionRequest) => {});
    const target = renderComponent(WorkflowVersionSourceEditor, {
      versions: [version('v1')],
      baseVersion: version('v1'),
      onValidateSource,
      onCreateVersion,
    });

    expect((byTestId(target, 'workflow-source-version') as HTMLInputElement).value).toBe('v2');
    expect((byTestId(target, 'workflow-source-create-version') as HTMLButtonElement).disabled).toBe(true);

    byTestId(target, 'workflow-source-validate').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-source-validation-ready').textContent).toContain('abc123');
    });
    expect(target.querySelector('[data-source-path="dev/workflows/demo/run.mjs"]')).toBeInstanceOf(HTMLButtonElement);
    expect((byTestId(target, 'workflow-source-create-version') as HTMLButtonElement).disabled).toBe(false);

    byTestId(target, 'workflow-source-create-version').click();

    await vi.waitFor(() => {
      expect(onCreateVersion).toHaveBeenCalledOnce();
    });
    expect(onCreateVersion.mock.calls[0]?.[0]).toMatchObject({
      version: 'v2',
      entrypoint: 'dev/workflows/demo/run.mjs',
      source: {
        resolved_commit: 'abc123',
      },
    });
  });

  it('shows field errors and validation failures near the source editor', async () => {
    const onValidateSource = vi.fn(async () => {
      throw new Error('git ref was not found');
    });
    const target = renderComponent(WorkflowVersionSourceEditor, {
      versions: [version('v1')],
      baseVersion: version('v1'),
      onValidateSource,
    });

    setInput(target, 'workflow-source-version', 'v1');

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-source-version-error').textContent).toContain('already exists');
    });
    expect((byTestId(target, 'workflow-source-validate') as HTMLButtonElement).disabled).toBe(true);

    setInput(target, 'workflow-source-version', 'v2');
    await vi.waitFor(() => {
      expect((byTestId(target, 'workflow-source-validate') as HTMLButtonElement).disabled).toBe(false);
    });
    byTestId(target, 'workflow-source-validate').click();

    await vi.waitFor(() => {
      expect(byTestId(target, 'workflow-source-validation-error').textContent).toContain('git ref was not found');
    });
  });
});

function setInput(target: ParentNode, testId: string, value: string): void {
  const input = byTestId(target, testId) as HTMLInputElement;
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

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

function sourceValidation() {
  return {
    workflow_definition_id: 'workflow-1',
    entrypoint: 'dev/workflows/demo/run.mjs',
    source: {
      kind: 'git' as const,
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    files: [
      {
        path: 'dev/workflows/demo/run.mjs',
        byte_count: 38,
        media_type: 'text/javascript; charset=utf-8',
        language: 'typescript',
        entrypoint: true,
      },
    ],
  };
}
