import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowRunOverview from './WorkflowRunOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowRunOverview', () => {
  it('renders ready metrics and refresh action', () => {
    const onRefresh = vi.fn();
    const target = renderComponent(WorkflowRunOverview, {
      state: { status: 'ready', runs: [workflowRun()] },
      onRefresh,
    });

    expect(byTestId(target, 'workflow-runs-metric-total').textContent).toContain('1');
    expect(byTestId(target, 'workflow-runs-list').textContent).toContain('workflow-1');
    expect(byTestId(target, 'workflow-runs-integration-panel').textContent).toContain('Start workflow runs from outside');
    expect(byTestId(target, 'workflow-runs-rest-example').textContent).toContain('POST /api/v1/workflow-runs');
    expect(byTestId(target, 'workflow-runs-cli-example').textContent).toContain('workflow run create');

    byTestId(target, 'workflow-runs-refresh').click();

    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it('renders loading, error, and empty states', async () => {
    const loadingTarget = renderComponent(WorkflowRunOverview, {
      state: { status: 'loading' },
    });
    expect(byTestId(loadingTarget, 'workflow-runs-loading').textContent).toContain('Loading workflow runs');
    await cleanupRenderedComponents();

    const errorTarget = renderComponent(WorkflowRunOverview, {
      state: { status: 'error', message: 'catalog down' },
    });
    expect(byTestId(errorTarget, 'workflow-runs-error').textContent).toContain('catalog down');
    await cleanupRenderedComponents();

    const emptyTarget = renderComponent(WorkflowRunOverview, {
      state: { status: 'ready', runs: [] },
    });
    expect(byTestId(emptyTarget, 'workflow-runs-empty').textContent).toContain('Workflow run catalog is empty');
  });
});

function workflowRun(): WorkflowRunResource {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'manual',
    source_reference: 'admin',
    client_request_id: 'request-1',
    state: 'running',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: {},
    output: null,
    error: null,
    artifact_refs: [],
    source_snapshot: null,
    extensions: [],
    credential_bindings: [],
    workspace_inputs: [],
    produced_files: [],
    recordings: [],
    retention: { logs_expire_at: null, output_expire_at: null },
    project_admission: null,
    admission: null,
    intervention: {},
    runtime: null,
    labels: {},
    started_at: '2026-06-21T10:01:00.000Z',
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:05:00.000Z',
  };
}
