import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEventSubscriptionDetailRoute from './WorkflowEventSubscriptionDetailRoute.svelte';
beforeEach(() =>
  window.history.replaceState(
    null,
    '',
    'http://localhost:3000/admin-new/workflow-event-subscriptions/subscription-1',
  ),
);
afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});
describe('WorkflowEventSubscriptionDetailRoute', () => {
  it('loads delivery diagnostics, refreshes, and deletes after confirmation', async () => {
    const navigateToCatalog = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/deliveries'))
        return jsonResponse({ deliveries: [deliveryPayload()] }, 200);
      return jsonResponse(subscriptionPayload(), init?.method === 'DELETE' ? 200 : 200);
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowEventSubscriptionDetailRoute, {
      authContext: authContext(),
      navigateToCatalog,
    });
    await vi.waitFor(() =>
      expect(byTestId(target, 'workflow-event-delivery-row').textContent).toContain(
        'receiver unavailable',
      ),
    );
    byTestId(target, 'workflow-event-subscription-refresh').click();
    await vi.waitFor(() =>
      expect(byTestId(target, 'workflow-event-subscription-action-success').textContent).toContain(
        'refreshed',
      ),
    );
    byTestId(target, 'workflow-event-subscription-delete').click();
    await tick();
    byTestId(target, 'workflow-event-subscription-delete-confirm-button').click();
    await vi.waitFor(() => expect(navigateToCatalog).toHaveBeenCalledOnce());
    expect(fetchImpl.mock.calls.some((call) => call[1]?.method === 'DELETE')).toBe(true);
  });
});
function authContext(): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: async () => 'shell-token',
    onAuthenticationFailure: vi.fn(),
    login: async () => {},
    logout: async () => {},
  };
}
function jsonResponse(payload: unknown, status: number) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
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
    state: 'failed',
    attempt_count: 2,
    next_attempt_at: null,
    last_attempt_at: '2026-08-07T09:00:00.000Z',
    delivered_at: null,
    last_response_status: 503,
    last_error: 'receiver unavailable',
    payload: { run_id: 'run-1' },
    attempts: [],
    created_at: '2026-08-07T08:59:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
