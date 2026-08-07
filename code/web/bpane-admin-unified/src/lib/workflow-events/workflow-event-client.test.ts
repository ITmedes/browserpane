import { describe, expect, it, vi } from 'vitest';
import { WorkflowEventCatalogClient, WorkflowEventCatalogError, toWorkflowEventDeliveryListResponse, toWorkflowEventSubscriptionListResponse } from './workflow-event-client';

describe('WorkflowEventCatalogClient', () => {
  it('manages subscriptions and loads delivery diagnostics through authenticated requests', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/workflow-event-subscriptions') && init?.method === 'GET') return jsonResponse({ subscriptions: [subscriptionPayload()] }, 200);
      if (url.endsWith('/deliveries')) return jsonResponse({ deliveries: [deliveryPayload()] }, 200);
      return jsonResponse(subscriptionPayload(), init?.method === 'POST' ? 201 : 200);
    });
    const client = new WorkflowEventCatalogClient({ baseUrl: 'http://browserpane.test', accessTokenProvider: () => 'token-1', fetchImpl });
    const request = { name: 'Workflow events', target_url: 'https://events.example/hook', event_types: ['workflow_run.*'], signing_secret: 'write-only-secret' };

    await client.listSubscriptions();
    const created = await client.createSubscription(request);
    await client.getSubscription('subscription-1');
    const deliveries = await client.listDeliveries('subscription-1');
    await client.deleteSubscription('subscription-1');

    expect(JSON.stringify(created)).not.toContain('write-only-secret');
    expect(deliveries.deliveries[0]).toMatchObject({ state: 'failed', attempt_count: 2, last_response_status: 503 });
    expect(fetchImpl.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(request));
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('discards unexpected signing-secret fields and rejects malformed resources', () => {
    const response = toWorkflowEventSubscriptionListResponse({ subscriptions: [{ ...subscriptionPayload(), signing_secret: 'must-not-leak' }] });
    expect(JSON.stringify(response)).not.toContain('must-not-leak');
    expect(() => toWorkflowEventSubscriptionListResponse({ subscriptions: {} })).toThrow(WorkflowEventCatalogError);
    expect(() => toWorkflowEventDeliveryListResponse({ deliveries: [{ ...deliveryPayload(), state: 'unknown' }] })).toThrow('must be one of');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new WorkflowEventCatalogClient({ baseUrl: 'http://browserpane.test', accessTokenProvider: () => 'expired', fetchImpl: async () => new Response('unauthorized', { status: 401 }), onAuthenticationFailure });
    await expect(client.listSubscriptions()).rejects.toMatchObject({ status: 401 });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function jsonResponse(payload: unknown, status: number) { return new Response(JSON.stringify(payload), { status, headers: { 'content-type': 'application/json' } }); }
function subscriptionPayload() { return { id: 'subscription-1', name: 'Workflow events', target_url: 'https://events.example/hook', event_types: ['workflow_run.*'], has_signing_secret: true, deliveries_path: '/api/v1/workflow-event-subscriptions/subscription-1/deliveries', created_at: '2026-08-07T08:00:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' }; }
function deliveryPayload() { return { id: 'delivery-1', subscription_id: 'subscription-1', run_id: 'run-1', event_id: 'event-1', event_type: 'workflow_run.failed', state: 'failed', attempt_count: 2, next_attempt_at: '2026-08-07T09:05:00.000Z', last_attempt_at: '2026-08-07T09:00:00.000Z', delivered_at: null, last_response_status: 503, last_error: 'receiver unavailable', payload: { run_id: 'run-1' }, attempts: [{ id: 'attempt-1', delivery_id: 'delivery-1', attempt_number: 1, response_status: 503, error: 'receiver unavailable', created_at: '2026-08-07T09:00:00.000Z' }], created_at: '2026-08-07T08:59:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' }; }
