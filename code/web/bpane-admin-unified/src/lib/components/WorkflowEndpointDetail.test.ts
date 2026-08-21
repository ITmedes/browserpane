import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEndpointDetail from './WorkflowEndpointDetail.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowEndpointDetail', () => {
  it('renders loading, denied, and ready contract evidence without secrets', async () => {
    let target = renderComponent(WorkflowEndpointDetail, { state: { status: 'loading', projectId: 'project-1', endpointKey: 'report' } });
    expect(byTestId(target, 'workflow-endpoint-detail-loading')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents();
    target = renderComponent(WorkflowEndpointDetail, { state: { status: 'error', projectId: 'project-1', endpointKey: 'report', message: 'Permission denied.' } });
    expect(byTestId(target, 'workflow-endpoint-detail-error').textContent).toContain('Permission denied');
    await cleanupRenderedComponents();
    target = renderComponent(WorkflowEndpointDetail, { state: { status: 'ready', endpoint: endpoint(), grants: [grant()] } });
    expect(byTestId(target, 'workflow-endpoint-detail-key').textContent).toContain('retrieve-report');
    expect(byTestId(target, 'workflow-endpoint-grant-row').textContent).toContain('principal-1');
    expect(byTestId(target, 'workflow-endpoint-invocation-example').textContent).toContain('$ACCESS_TOKEN');
    expect(target.textContent).not.toContain('client-secret-value');
  });

  it('delegates activation, grant, revoke, and stale refresh actions', async () => {
    const onActivate = vi.fn();
    const onGrant = vi.fn();
    const onRevoke = vi.fn();
    const target = renderComponent(WorkflowEndpointDetail, {
      state: { status: 'ready', endpoint: endpoint(), grants: [grant()] },
      actionState: { status: 'running', label: 'Refreshing stale endpoint...' },
      onActivate,
      onGrant,
      onRevoke,
    });
    expect(byTestId(target, 'workflow-endpoint-action-running').textContent).toContain('Refreshing stale');
    (byTestId(target, 'workflow-endpoint-activate') as HTMLButtonElement).disabled = false;
    byTestId(target, 'workflow-endpoint-activate').click();
    expect(onActivate).toHaveBeenCalledOnce();
    (byTestId(target, 'workflow-endpoint-grant-submit') as HTMLButtonElement).disabled = false;
    byTestId(target, 'workflow-endpoint-grant-submit').click();
    await tick();
    expect(onGrant).not.toHaveBeenCalled();
    expect(target.textContent).toContain('Service principal id is required');
    const principal = byTestId(target, 'workflow-endpoint-grant-principal') as HTMLInputElement;
    principal.value = 'principal-2';
    principal.dispatchEvent(new Event('input', { bubbles: true }));
    byTestId(target, 'workflow-endpoint-grant-submit').click();
    await tick();
    expect(onGrant).toHaveBeenCalledWith({ service_principal_id: 'principal-2', operations: ['invoke', 'read'] });
    const revoke = byTestId(target, 'workflow-endpoint-grant-row').querySelector('button')!;
    revoke.removeAttribute('disabled');
    revoke.click();
    expect(onRevoke).toHaveBeenCalledWith('grant-1');
  });

  it('shows disablement for active endpoints and success feedback', () => {
    const onDisable = vi.fn();
    const target = renderComponent(WorkflowEndpointDetail, {
      state: { status: 'ready', endpoint: { ...endpoint(), state: 'active' }, grants: [] },
      actionState: { status: 'success', message: 'Endpoint activated.' },
      onDisable,
    });
    byTestId(target, 'workflow-endpoint-disable').click();
    expect(onDisable).toHaveBeenCalledOnce();
    expect(byTestId(target, 'workflow-endpoint-action-success').textContent).toContain('activated');
  });
});

function endpoint() {
  return {
    id: 'endpoint-1',
    project_id: 'project-1',
    endpoint_key: 'retrieve-report',
    purpose: 'Retrieve a supplier report.',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'version-1',
    workflow_version: '1.0.0',
    input_schema: { type: 'object', properties: { reporting_period: { type: 'string' } } },
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

function grant() {
  return {
    id: 'grant-1',
    endpoint_id: 'endpoint-1',
    project_id: 'project-1',
    service_principal_id: 'principal-1',
    operations: ['invoke', 'read'] as const,
    created_at: '2026-08-21T08:00:00.000Z',
    updated_at: '2026-08-21T08:00:00.000Z',
  };
}
