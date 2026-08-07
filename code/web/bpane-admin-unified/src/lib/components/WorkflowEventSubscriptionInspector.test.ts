import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEventSubscriptionInspector from './WorkflowEventSubscriptionInspector.svelte';

afterEach(cleanupRenderedComponents);
describe('WorkflowEventSubscriptionInspector', () => {
  it('renders delivery health and attempts without a signing secret', () => {
    const target = renderComponent(WorkflowEventSubscriptionInspector, {
      state: {
        status: 'ready',
        subscription: subscriptionPayload(),
        deliveries: [deliveryPayload()],
      },
    });
    expect(byTestId(target, 'workflow-events-metric-failed').textContent).toContain('1');
    expect(byTestId(target, 'workflow-event-delivery-row').textContent).toContain('HTTP 503');
    expect(byTestId(target, 'workflow-event-delivery-row').textContent).toContain(
      'receiver unavailable',
    );
    expect(target.textContent).not.toContain('write-only-secret');
  });
  it('requires inline confirmation before deletion', async () => {
    const onDelete = vi.fn();
    const target = renderComponent(WorkflowEventSubscriptionInspector, {
      state: { status: 'ready', subscription: subscriptionPayload(), deliveries: [] },
      onDelete,
    });
    byTestId(target, 'workflow-event-subscription-delete').click();
    await tick();
    expect(byTestId(target, 'workflow-event-subscription-delete-confirm')).toBeInstanceOf(
      HTMLElement,
    );
    expect(onDelete).not.toHaveBeenCalled();
    byTestId(target, 'workflow-event-subscription-delete-confirm-button').click();
    expect(onDelete).toHaveBeenCalledOnce();
  });
});
function subscriptionPayload() {
  return {
    id: 'subscription-1',
    name: 'Workflow events',
    target_url: 'https://events.example/hook',
    event_types: ['workflow_run.*'],
    has_signing_secret: true,
    deliveries_path: '/api/v1/workflow-event-subscriptions/subscription-1/deliveries',
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
function deliveryPayload() {
  return {
    id: 'delivery-1',
    subscription_id: 'subscription-1',
    run_id: 'run-1',
    event_id: 'event-1',
    event_type: 'workflow_run.failed',
    state: 'failed' as const,
    attempt_count: 2,
    next_attempt_at: '2026-08-07T09:05:00.000Z',
    last_attempt_at: '2026-08-07T09:00:00.000Z',
    delivered_at: null,
    last_response_status: 503,
    last_error: 'receiver unavailable',
    payload: { run_id: 'run-1' },
    attempts: [
      {
        id: 'attempt-1',
        delivery_id: 'delivery-1',
        attempt_number: 1,
        response_status: 503,
        error: 'receiver unavailable',
        created_at: '2026-08-07T09:00:00.000Z',
      },
    ],
    created_at: '2026-08-07T08:59:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
