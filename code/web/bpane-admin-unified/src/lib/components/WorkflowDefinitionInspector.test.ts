import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
} from '$lib/workflows/workflow-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowDefinitionInspector from './WorkflowDefinitionInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowDefinitionInspector', () => {
  it('renders definition metadata and delegates refresh', () => {
    const onRefreshWorkflow = vi.fn();
    const target = renderComponent(WorkflowDefinitionInspector, {
      state: { status: 'ready', definition: workflow(), versions: [version()] },
      sourcePreviewState: {
        status: 'ready',
        version: 'v1',
        preview: sourcePreview(),
      },
      sourceFilesState: sourceFilesState(),
      onRefreshWorkflow,
    });

    expect(byTestId(target, 'workflow-definition-detail-title').textContent).toContain('BrowserPane Tour');
    expect(byTestId(target, 'workflow-definition-detail-kind').textContent).toContain('Example template');
    expect(byTestId(target, 'workflow-definition-detail-latest-version').textContent).toContain('v1');
    expect(byTestId(target, 'workflow-definition-version-entrypoint').textContent).toContain('browserpane-tour');
    expect(byTestId(target, 'workflow-definition-source').textContent).toContain('/workspace');
    expect(byTestId(target, 'workflow-definition-policy').textContent).toContain('workspace-1');
    expect(target.querySelector('[data-source-path="dev/workflows/browserpane-tour/run.mjs"]')).toBeInstanceOf(HTMLButtonElement);
    expect(byTestId(target, 'workflow-code-preview-code').textContent).toContain('export default async function run');

    byTestId(target, 'workflow-definition-refresh-detail').click();

    expect(onRefreshWorkflow).toHaveBeenCalledOnce();
  });

  it('renders idle, loading, and error states', () => {
    let target = renderComponent(WorkflowDefinitionInspector, { state: { status: 'idle' } });
    expect(byTestId(target, 'workflow-definition-inspector-idle').textContent).toContain('Select a workflow');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowDefinitionInspector, { state: { status: 'loading', workflowId: 'workflow-1' } });
    expect(byTestId(target, 'workflow-definition-inspector-loading').textContent).toContain('Loading workflow');

    void cleanupRenderedComponents();
    target = renderComponent(WorkflowDefinitionInspector, {
      state: { status: 'error', workflowId: 'workflow-1', message: 'Missing workflow.' },
    });
    expect(byTestId(target, 'workflow-definition-inspector-error').textContent).toContain('Missing workflow');
  });
});

function workflow(): WorkflowDefinitionResource {
  return {
    id: 'workflow-1',
    name: 'BrowserPane Tour',
    description: 'Example tour',
    labels: { bpane_admin_template: 'browserpane-tour' },
    latest_version: 'v1',
    created_at: '2026-06-21T09:00:00.000Z',
    updated_at: '2026-06-21T10:00:00.000Z',
  };
}

function sourcePreview() {
  return {
    workflow_definition_id: 'workflow-1',
    workflow_version: 'v1',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    path: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git' as const,
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    media_type: 'text/javascript; charset=utf-8',
    language: 'typescript',
    content: 'export default async function run() {}',
    byte_count: 38,
    max_bytes: 65536,
    truncated: false,
  };
}

function sourceFilesState() {
  return {
    status: 'ready' as const,
    version: 'v1',
    response: {
      workflow_definition_id: 'workflow-1',
      workflow_version: 'v1',
      entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
      source: {
        kind: 'git' as const,
        repository_url: '/workspace',
        ref: 'HEAD',
        resolved_commit: 'abc123',
        root_path: 'dev',
      },
      files: [
        {
          path: 'dev/workflows/browserpane-tour/run.mjs',
          byte_count: 38,
          media_type: 'text/javascript; charset=utf-8',
          language: 'typescript',
          entrypoint: true,
        },
      ],
    },
  };
}

function version(): WorkflowDefinitionVersionResource {
  return {
    id: 'version-1',
    workflow_definition_id: 'workflow-1',
    version: 'v1',
    executor: 'playwright',
    entrypoint: 'dev/workflows/browserpane-tour/run.mjs',
    source: {
      kind: 'git',
      repository_url: '/workspace',
      ref: 'HEAD',
      resolved_commit: 'abc123',
      root_path: 'dev',
    },
    input_schema: { type: 'object' },
    output_schema: null,
    default_session: null,
    allowed_credential_binding_ids: ['credential-1'],
    allowed_extension_ids: [],
    allowed_file_workspace_ids: ['workspace-1'],
    created_at: '2026-06-21T09:30:00.000Z',
  };
}
