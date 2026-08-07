import { describe, expect, it } from 'vitest';
import { buildWorkflowEventDeliverySummary, buildWorkflowEventOverviewModel, createWorkflowEventSubscriptionDraft, validateWorkflowEventSubscriptionDraft, workflowEventSubscriptionMatchesSearch } from './workflow-event-view-model';
import type { WorkflowEventDeliveryResource, WorkflowEventSubscriptionResource } from './workflow-event-types';

describe('workflow event view model', () => {
  it('summarizes subscription signing and filters', () => {
    const model = buildWorkflowEventOverviewModel([subscription()]);
    expect(model.metrics.map((metric) => [metric.label, metric.value])).toEqual([['Subscriptions', '1'], ['Signed', '1'], ['Wildcard filters', '1'], ['Event filters', '1']]);
    expect(workflowEventSubscriptionMatchesSearch(model.rows[0]!, 'workflow_run.*')).toBe(true);
  });
  it('validates subscription input and keeps signing secret write-only in the request', () => {
    const draft = createWorkflowEventSubscriptionDraft();
    Object.assign(draft, { name: 'Workflow events', targetUrl: 'https://events.example/hook', eventTypesText: 'workflow_run.created,workflow_run.failed', signingSecret: 'secret-value' });
    expect(validateWorkflowEventSubscriptionDraft(draft).request).toEqual({ name: 'Workflow events', target_url: 'https://events.example/hook', event_types: ['workflow_run.created', 'workflow_run.failed'], signing_secret: 'secret-value' });
    Object.assign(draft, { targetUrl: 'ftp://invalid', eventTypesText: 'bad event', signingSecret: '' });
    expect(validateWorkflowEventSubscriptionDraft(draft)).toMatchObject({ valid: false, fieldErrors: { targetUrl: expect.any(Array), eventTypes: expect.any(Array), signingSecret: expect.any(Array) } });
  });
  it('summarizes failed delivery diagnostics and attempts', () => {
    const model = buildWorkflowEventDeliverySummary([delivery()]);
    expect(model.metrics.map((metric) => metric.value)).toEqual(['1', '0', '0', '1']);
    expect(model.rows[0]).toMatchObject({ state: 'Failed', attempts: '2 attempts', response: 'HTTP 503', error: 'receiver unavailable', runId: 'run-1' });
    expect(model.rows[0]?.attemptsDetail[0]).toMatchObject({ label: 'Attempt 1', response: 'HTTP 503' });
  });
});

function subscription(): WorkflowEventSubscriptionResource { return { id: 'subscription-1', name: 'Workflow events', target_url: 'https://events.example/hook', event_types: ['workflow_run.*'], has_signing_secret: true, deliveries_path: '/api/v1/workflow-event-subscriptions/subscription-1/deliveries', created_at: '2026-08-07T08:00:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' }; }
function delivery(): WorkflowEventDeliveryResource { return { id: 'delivery-1', subscription_id: 'subscription-1', run_id: 'run-1', event_id: 'event-1', event_type: 'workflow_run.failed', state: 'failed', attempt_count: 2, next_attempt_at: '2026-08-07T09:05:00.000Z', last_attempt_at: '2026-08-07T09:00:00.000Z', delivered_at: null, last_response_status: 503, last_error: 'receiver unavailable', payload: { run_id: 'run-1' }, attempts: [{ id: 'attempt-1', delivery_id: 'delivery-1', attempt_number: 1, response_status: 503, error: 'receiver unavailable', created_at: '2026-08-07T09:00:00.000Z' }], created_at: '2026-08-07T08:59:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' }; }
