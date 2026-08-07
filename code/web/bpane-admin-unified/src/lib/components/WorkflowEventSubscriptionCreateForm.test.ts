import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEventSubscriptionCreateForm from './WorkflowEventSubscriptionCreateForm.svelte';

afterEach(cleanupRenderedComponents);
describe('WorkflowEventSubscriptionCreateForm', () => {
  it('validates target, event types, and signing secret near controls', async () => {
    const target = renderComponent(WorkflowEventSubscriptionCreateForm);
    await input(target, 'workflow-event-subscription-name', 'Events');
    await input(target, 'workflow-event-subscription-target', 'ftp://invalid');
    await input(target, 'workflow-event-subscription-event-types', 'bad event');
    expect(byTestId(target, 'workflow-event-subscription-target-error').textContent).toContain(
      'HTTP or HTTPS',
    );
    expect(byTestId(target, 'workflow-event-subscription-event-types-error').textContent).toContain(
      'spaces',
    );
    expect(
      byTestId(target, 'workflow-event-subscription-signing-secret-error').textContent,
    ).toContain('required');
  });
  it('submits normalized input with a write-only signing secret', async () => {
    const onSave = vi.fn();
    const target = renderComponent(WorkflowEventSubscriptionCreateForm, { onSave });
    await input(target, 'workflow-event-subscription-name', ' Workflow events ');
    await input(target, 'workflow-event-subscription-target', 'https://events.example/hook');
    await input(
      target,
      'workflow-event-subscription-event-types',
      'workflow_run.created\nworkflow_run.failed',
    );
    await input(target, 'workflow-event-subscription-signing-secret', 'secret-value');
    byTestId(target, 'workflow-event-subscription-create-submit').click();
    expect(onSave).toHaveBeenCalledWith({
      name: 'Workflow events',
      target_url: 'https://events.example/hook',
      event_types: ['workflow_run.created', 'workflow_run.failed'],
      signing_secret: 'secret-value',
    });
  });
});
async function input(target: HTMLElement, testId: string, value: string) {
  const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}
