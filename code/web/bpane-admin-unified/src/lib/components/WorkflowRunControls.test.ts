import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import { workflowRunFixture } from '$lib/test-utils/workflow-run-fixture';
import { workflowRunControlAvailability } from '$lib/workflow-runs/workflow-run-detail-view-model';
import WorkflowRunControls from './WorkflowRunControls.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowRunControls', () => {
  it('submits parsed JSON and a trimmed rejection reason', async () => {
    const onSubmitInput = vi.fn();
    const onReject = vi.fn();
    const run = workflowRunFixture();
    const target = renderComponent(WorkflowRunControls, {
      availability: workflowRunControlAvailability(run),
      pendingRequest: run.intervention.pending_request,
      onSubmitInput,
      onReject,
    });

    setInputValue(byTestId(target, 'workflow-run-detail-operator-input'), '{"approved":true}');
    setInputValue(byTestId(target, 'workflow-run-detail-reject-reason'), '  policy denied  ');
    await tick();
    byTestId(target, 'workflow-run-detail-submit-input').click();
    byTestId(target, 'workflow-run-detail-reject').click();

    expect(onSubmitInput).toHaveBeenCalledWith({ approved: true });
    expect(onReject).toHaveBeenCalledWith('policy denied');
  });

  it('keeps invalid JSON visible and disables submission', async () => {
    const run = workflowRunFixture();
    const target = renderComponent(WorkflowRunControls, {
      availability: workflowRunControlAvailability(run),
      pendingRequest: run.intervention.pending_request,
    });

    const input = byTestId(target, 'workflow-run-detail-operator-input');
    setInputValue(input, '{invalid');
    await tick();

    expect(input.getAttribute('aria-invalid')).toBe('true');
    expect(input.getAttribute('aria-describedby')).toBe('workflow-run-operator-input-help');
    expect(byTestId(target, 'workflow-run-detail-operator-input-help').textContent).toContain(
      'Enter valid JSON',
    );
    expect((byTestId(target, 'workflow-run-detail-submit-input') as HTMLButtonElement).disabled).toBe(true);
    expect((input as HTMLTextAreaElement).value).toBe('{invalid');
  });

  it('disables mutation controls for terminal runs and renders action feedback', () => {
    const run = workflowRunFixture({
      state: 'succeeded',
      intervention: { pending_request: null, last_resolution: null },
    });
    const target = renderComponent(WorkflowRunControls, {
      availability: workflowRunControlAvailability(run),
      pendingRequest: run.intervention.pending_request,
      actionState: { status: 'error', message: 'Conflict while updating the run.' },
    });

    expect((byTestId(target, 'workflow-run-detail-cancel') as HTMLButtonElement).disabled).toBe(true);
    expect((byTestId(target, 'workflow-run-detail-resume') as HTMLButtonElement).disabled).toBe(true);
    expect((byTestId(target, 'workflow-run-detail-reject') as HTMLButtonElement).disabled).toBe(true);
    expect(byTestId(target, 'workflow-run-detail-action-error').textContent).toContain('Conflict');
  });
});

function setInputValue(element: Element, value: string): void {
  (element as HTMLInputElement | HTMLTextAreaElement).value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
}
