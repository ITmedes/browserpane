import { afterEach, describe, expect, it, vi } from 'vitest';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import WorkflowEventSubscriptionOverview from './WorkflowEventSubscriptionOverview.svelte';

afterEach(cleanupRenderedComponents);
describe('WorkflowEventSubscriptionOverview', () => {
  it('renders loading, error, empty, and ready states', async () => {
    let target = renderComponent(WorkflowEventSubscriptionOverview, { state: { status: 'loading' } }); expect(byTestId(target, 'workflow-event-subscriptions-loading')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents(); target = renderComponent(WorkflowEventSubscriptionOverview, { state: { status: 'error', message: 'Unavailable.' } }); expect(byTestId(target, 'workflow-event-subscriptions-error').textContent).toContain('Unavailable');
    await cleanupRenderedComponents(); target = renderComponent(WorkflowEventSubscriptionOverview, { state: { status: 'ready', subscriptions: [] } }); expect(byTestId(target, 'workflow-event-subscriptions-empty')).toBeInstanceOf(HTMLElement);
    await cleanupRenderedComponents(); target = renderComponent(WorkflowEventSubscriptionOverview, { state: { status: 'ready', subscriptions: [subscriptionPayload()] } }); expect(byTestId(target, 'workflow-events-metric-total').textContent).toContain('1');
  });
  it('delegates refresh', () => { const onRefresh = vi.fn(); const target = renderComponent(WorkflowEventSubscriptionOverview, { state: { status: 'ready', subscriptions: [subscriptionPayload()] }, onRefresh }); byTestId(target, 'workflow-event-subscriptions-refresh-button').click(); expect(onRefresh).toHaveBeenCalledOnce(); });
});
function subscriptionPayload() { return { id: 'subscription-1', name: 'Workflow events', target_url: 'https://events.example/hook', event_types: ['workflow_run.*'], has_signing_secret: true, deliveries_path: '/api/v1/workflow-event-subscriptions/subscription-1/deliveries', created_at: '2026-08-07T08:00:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' }; }
