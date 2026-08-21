import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEndpointOverview from './WorkflowEndpointOverview.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowEndpointOverview', () => {
  it('renders loading, denied, empty, and ready catalog states', async () => {
    let target = renderComponent(WorkflowEndpointOverview, baseProps({ status: 'loading' }));
    expect(byTestId(target, 'workflow-endpoints-loading')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents();
    target = renderComponent(WorkflowEndpointOverview, baseProps({ status: 'error', message: 'Access denied.' }));
    expect(byTestId(target, 'workflow-endpoints-error').textContent).toContain('Access denied');
    await cleanupRenderedComponents();
    target = renderComponent(WorkflowEndpointOverview, baseProps({ status: 'ready', projectId: 'project-1', workflowEndpoints: [] }));
    expect(byTestId(target, 'workflow-endpoints-empty')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents();
    target = renderComponent(WorkflowEndpointOverview, baseProps({ status: 'ready', projectId: 'project-1', workflowEndpoints: [endpoint()] }));
    expect(byTestId(target, 'workflow-endpoint-row').textContent).toContain('retrieve-report');
  });

  it('shows invalid create feedback without submitting and exposes action progress', async () => {
    const onCreate = vi.fn();
    let target = renderComponent(WorkflowEndpointOverview, {
      ...baseProps({ status: 'ready', projectId: 'project-1', workflowEndpoints: [] }),
      actionState: { status: 'running', label: 'Creating endpoint...' },
      onCreate,
    });
    expect(byTestId(target, 'workflow-endpoints-action-running').textContent).toContain('Creating');
    await cleanupRenderedComponents();
    target = renderComponent(WorkflowEndpointOverview, {
      ...baseProps({ status: 'ready', projectId: 'project-1', workflowEndpoints: [] }),
      onCreate,
    });
    byTestId(target, 'workflow-endpoints-new-button').click();
    await tick();
    byTestId(target, 'workflow-endpoint-create-submit').click();
    await tick();
    expect(onCreate).not.toHaveBeenCalled();
    expect(target.textContent).toContain('Use 1-64 lowercase letters');
  });

  it('delegates project selection and refresh', () => {
    const onSelectProject = vi.fn();
    const onRefresh = vi.fn();
    const target = renderComponent(WorkflowEndpointOverview, {
      ...baseProps({ status: 'ready', projectId: 'project-1', workflowEndpoints: [] }),
      onSelectProject,
      onRefresh,
    });
    const select = byTestId(target, 'workflow-endpoints-project-select') as HTMLSelectElement;
    select.value = 'project-2';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    byTestId(target, 'workflow-endpoints-refresh-button').click();
    expect(onSelectProject).toHaveBeenCalledWith('project-2');
    expect(onRefresh).toHaveBeenCalledOnce();
  });
});

function baseProps(state: unknown) {
  return {
    state,
    projects: [
      { id: 'project-1', name: 'Supplier', state: 'active' },
      { id: 'project-2', name: 'Finance', state: 'active' },
    ],
    selectedProjectId: 'project-1',
  };
}

function endpoint() {
  return {
    id: 'endpoint-1',
    project_id: 'project-1',
    endpoint_key: 'retrieve-report',
    purpose: 'Retrieve a supplier report.',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'version-1',
    workflow_version: '1.0.0',
    input_schema: { type: 'object' },
    output_schema: { type: 'object' },
    execution_timeout_seconds: 900,
    inline_result_max_bytes: 65_536,
    artifact_behavior: { mode: 'authorized_references' as const, retention_seconds: 86_400 },
    supported_controls: ['poll', 'cancel'],
    labels: {},
    state: 'draft' as const,
    grants_path: '/grants',
    invocations_path: '/invocations',
    created_at: '2026-08-21T08:00:00.000Z',
    updated_at: '2026-08-21T08:00:00.000Z',
  };
}
