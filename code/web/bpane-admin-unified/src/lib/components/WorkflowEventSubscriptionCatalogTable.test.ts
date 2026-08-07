import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import type { WorkflowEventSubscriptionResource } from '$lib/workflow-events/workflow-event-types';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEventSubscriptionCatalogTable from './WorkflowEventSubscriptionCatalogTable.svelte';

afterEach(cleanupRenderedComponents);
describe('WorkflowEventSubscriptionCatalogTable', () => {
  it('filters wildcard and explicit subscriptions and searches targets', async () => {
    const target = renderComponent(WorkflowEventSubscriptionCatalogTable, {
      subscriptions: [
        subscription('wildcard', ['workflow_run.*']),
        subscription('explicit', ['workflow_run.failed']),
      ],
    });
    byTestId(target, 'workflow-event-subscriptions-lens-wildcard').click();
    await tick();
    expect(
      target.querySelectorAll('[data-testid="workflow-event-subscriptions-list-row"]'),
    ).toHaveLength(1);
    expect(target.textContent).toContain('wildcard');
    byTestId(target, 'workflow-event-subscriptions-lens-all').click();
    await input(target, 'workflow-event-subscriptions-search', 'explicit.example');
    expect(
      target.querySelectorAll('[data-testid="workflow-event-subscriptions-list-row"]'),
    ).toHaveLength(1);
    expect(target.textContent).toContain('explicit');
  });
});
async function input(target: HTMLElement, testId: string, value: string) {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}
function subscription(id: string, eventTypes: string[]): WorkflowEventSubscriptionResource {
  return {
    id,
    name: id,
    target_url: `https://${id}.example/hook`,
    event_types: eventTypes,
    has_signing_secret: true,
    deliveries_path: `/api/v1/workflow-event-subscriptions/${id}/deliveries`,
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
