import { afterEach, describe, expect, it } from 'vitest';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEventDeliveryDiagnostics from './WorkflowEventDeliveryDiagnostics.svelte';

afterEach(cleanupRenderedComponents);

describe('WorkflowEventDeliveryDiagnostics', () => {
  it('renders failed delivery response and attempt details', () => {
    const target = renderComponent(WorkflowEventDeliveryDiagnostics, {
      deliveries: [
        {
          id: 'delivery-1',
          subscription_id: 'subscription-1',
          run_id: 'run-1',
          event_id: 'event-1',
          event_type: 'workflow_run.failed',
          state: 'failed',
          attempt_count: 1,
          next_attempt_at: null,
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
        },
      ],
    });

    expect(byTestId(target, 'workflow-events-metric-failed').textContent).toContain('1');
    expect(byTestId(target, 'workflow-event-delivery-row').textContent).toContain('HTTP 503');
    expect(byTestId(target, 'workflow-event-delivery-row').textContent).toContain(
      'receiver unavailable',
    );
  });

  it('renders an explicit empty state', () => {
    const target = renderComponent(WorkflowEventDeliveryDiagnostics, { deliveries: [] });

    expect(byTestId(target, 'workflow-event-deliveries-empty').textContent).toContain(
      'No deliveries',
    );
  });
});
