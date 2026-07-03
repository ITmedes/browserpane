import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';

import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowRunCatalogTable from './WorkflowRunCatalogTable.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowRunCatalogTable', () => {
  it('renders workflow run rows and filters by lens and search text', async () => {
    const target = renderComponent(WorkflowRunCatalogTable, {
      runs: [
        workflowRun({ id: 'run-active', state: 'running', sessionId: 'session-active' }),
        workflowRun({ id: 'run-failed', state: 'failed', sessionId: 'session-failed', error: 'worker failed' }),
      ],
    });

    expect(byTestId(target, 'workflow-runs-list-count').textContent).toContain('2 of 2');
    expect(target.querySelectorAll('[data-testid="workflow-runs-list-row"]')).toHaveLength(2);
    expect((byTestId(target, 'workflow-runs-workflow-link') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/workflows/workflow-1',
    );
    expect((byTestId(target, 'workflow-runs-session-link') as HTMLAnchorElement).href).toBe(
      'http://localhost:3000/admin-new/sessions/session-active',
    );

    byTestId(target, 'workflow-runs-lens-failed').click();
    await tick();

    expect(byTestId(target, 'workflow-runs-list-count').textContent).toContain('1 of 2');
    expect(byTestId(target, 'workflow-runs-list').textContent).toContain('worker failed');
    expect(byTestId(target, 'workflow-runs-list').textContent).not.toContain('session-active');

    byTestId(target, 'workflow-runs-lens-all').click();
    const search = byTestId(target, 'workflow-runs-search') as HTMLInputElement;
    search.value = 'missing text';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    expect(byTestId(target, 'workflow-runs-filter-empty').textContent).toContain('No workflow runs match');
  });
});

function workflowRun(overrides: Partial<{
  readonly id: string;
  readonly state: string;
  readonly sessionId: string;
  readonly error: string | null;
}> = {}): WorkflowRunResource {
  const id = overrides.id ?? 'run-1';
  return {
    id,
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'manual',
    source_reference: 'admin',
    client_request_id: 'request-1',
    state: overrides.state ?? 'running',
    session_id: overrides.sessionId ?? 'session-1',
    automation_task_id: 'task-1',
    input: {},
    output: null,
    error: overrides.error ?? null,
    artifact_refs: [],
    produced_files: [],
    project_admission: {
      state: 'allowed',
      reason_code: 'policy_ok',
      message: 'Workflow run allowed.',
      checked_at: '2026-06-21T10:00:00.000Z',
    },
    admission: null,
    intervention: {},
    runtime: null,
    labels: { suite: 'workflow-runs' },
    started_at: '2026-06-21T10:01:00.000Z',
    completed_at: overrides.state === 'failed' ? '2026-06-21T10:05:00.000Z' : null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:05:00.000Z',
  };
}
