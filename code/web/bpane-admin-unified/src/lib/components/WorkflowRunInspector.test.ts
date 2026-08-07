import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import {
  workflowRunEventFixture,
  workflowRunFixture,
  workflowRunLogFixture,
  workflowRunProducedFileFixture,
} from '$lib/test-utils/workflow-run-fixture';
import WorkflowRunInspector from './WorkflowRunInspector.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowRunInspector', () => {
  it('renders complete safe run context and canonical related links', () => {
    const target = renderComponent(WorkflowRunInspector, {
      run: workflowRunFixture(),
      evidence: evidence(),
    });

    expect(byTestId(target, 'workflow-run-detail-title').textContent).toContain('run-1');
    expect(byTestId(target, 'workflow-run-detail-session-id').textContent).toBe('session-1');
    expect(byTestId(target, 'workflow-run-detail-input').textContent).toContain('example.com');
    expect(byTestId(target, 'workflow-run-detail-workspace-inputs').textContent).toContain('customers.csv');
    expect(byTestId(target, 'workflow-run-detail-extensions').textContent).toContain('Password manager');
    expect(byTestId(target, 'workflow-run-detail-credential-bindings').textContent).toContain('vault_kv2');
    expect(byTestId(target, 'workflow-run-detail-recordings').textContent).toContain('recording-1');
    expect(byTestId(target, 'workflow-run-detail-workflow-link').getAttribute('href'))
      .toBe('/admin-new/workflows/workflow-1');
    expect(byTestId(target, 'workflow-run-detail-session-link').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1');
    expect(byTestId(target, 'workflow-run-detail-preview-link').getAttribute('href'))
      .toBe('/admin-new/sessions/session-1/preview');
  });

  it('delegates refresh, cancellation, and evidence download', () => {
    const onRefresh = vi.fn();
    const onCancel = vi.fn();
    const onDownloadProducedFile = vi.fn();
    const target = renderComponent(WorkflowRunInspector, {
      run: workflowRunFixture(),
      evidence: evidence(),
      onRefresh,
      onCancel,
      onDownloadProducedFile,
    });

    byTestId(target, 'workflow-run-detail-refresh').click();
    byTestId(target, 'workflow-run-detail-cancel').click();
    byTestId(target, 'workflow-run-detail-download-produced-file').click();

    expect(onRefresh).toHaveBeenCalledOnce();
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onDownloadProducedFile).toHaveBeenCalledWith(workflowRunProducedFileFixture());
  });
});

function evidence() {
  return {
    events: { status: 'ready' as const, items: [workflowRunEventFixture()] },
    logs: { status: 'ready' as const, items: [workflowRunLogFixture()] },
    producedFiles: { status: 'ready' as const, items: [workflowRunProducedFileFixture()] },
  };
}
