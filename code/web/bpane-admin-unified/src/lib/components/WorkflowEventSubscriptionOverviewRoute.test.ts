import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import WorkflowEventSubscriptionOverviewRoute from './WorkflowEventSubscriptionOverviewRoute.svelte';
beforeEach(() =>
  window.history.replaceState(
    null,
    '',
    'http://localhost:3000/admin-new/workflow-event-subscriptions',
  ),
);
afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});
describe('WorkflowEventSubscriptionOverviewRoute', () => {
  it('loads subscriptions through authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      jsonResponse({ subscriptions: [subscriptionPayload()] }, 200),
    );
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(WorkflowEventSubscriptionOverviewRoute, {
      authContext: authContext(),
    });
    await vi.waitFor(() =>
      expect(byTestId(target, 'workflow-event-subscriptions-list').textContent).toContain(
        'Workflow events',
      ),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
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
