import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { identityAccessReviewFixture } from '$lib/test-utils/identity-fixtures';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import IdentityAccessReviewRoute from './IdentityAccessReviewRoute.svelte';

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('IdentityAccessReviewRoute', () => {
  it('loads the deep-link route and refreshes coherent lifecycle mutations', async () => {
    let review = identityAccessReviewFixture();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = new URL(String(input));
      const headers = new Headers(init?.headers);
      expect(headers.get('authorization')).toBe('Bearer owner-token');
      if (url.pathname === '/api/v1/identity/access-review') {
        return jsonResponse(review);
      }
      if (url.pathname === '/api/v1/service-principals/principal-1' && init?.method === 'PUT') {
        const request = JSON.parse(String(init.body));
        review = {
          ...review,
          service_principals: review.service_principals.map((entry) => entry.id === 'principal-1'
            ? { ...entry, state: request.state }
            : entry),
        };
        return jsonResponse(review.service_principals[0]);
      }
      if (url.pathname === '/api/v1/identity-mappings/mapping-1' && init?.method === 'PUT') {
        const request = JSON.parse(String(init.body));
        review = {
          ...review,
          identity_mappings: review.identity_mappings.map((entry) => entry.id === 'mapping-1'
            ? { ...entry, state: request.state, effective_for_principal: request.state === 'active' }
            : entry),
        };
        return jsonResponse(review.identity_mappings[0]);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(IdentityAccessReviewRoute, { authContext: authContext() });

    await vi.waitFor(() => expect(byTestId(target, 'identity-principal-name').textContent).toContain('Demo Operator'));
    byTestId(target, 'identity-area-service-principals').click();
    await vi.waitFor(() => expect(byTestId(target, 'service-principal-disable')).toBeTruthy());
    byTestId(target, 'service-principal-disable').click();
    await vi.waitFor(() => expect(byTestId(target, 'identity-action-success').textContent).toContain('updated'));
    expect(byTestId(target, 'service-principal-enable')).toBeTruthy();

    byTestId(target, 'identity-area-mappings').click();
    await vi.waitFor(() => expect(byTestId(target, 'identity-mapping-disable')).toBeTruthy());
    byTestId(target, 'identity-mapping-disable').click();
    await vi.waitFor(() => expect(byTestId(target, 'identity-mapping-enable')).toBeTruthy());
    expect(byTestId(target, 'identity-action-success').textContent).toContain('updated');

    expect(fetchImpl.mock.calls.map((call) => [new URL(String(call[0])).pathname, call[1]?.method ?? 'GET']))
      .toContainEqual(['/api/v1/service-principals/principal-1', 'PUT']);
    expect(fetchImpl.mock.calls.filter((call) => String(call[0]).endsWith('/identity/access-review')).length).toBe(3);
  });

  it('surfaces malformed payloads and authentication failure through established handlers', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => jsonResponse({ projects: [] })));
    const malformed = renderComponent(IdentityAccessReviewRoute, { authContext: authContext() });
    await vi.waitFor(() => expect(byTestId(malformed, 'identity-load-error').textContent).toContain('must be an object'));
    await cleanupRenderedComponents();

    const onAuthenticationFailure = vi.fn();
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => new Response('unauthorized', { status: 401 })));
    const unauthorized = renderComponent(IdentityAccessReviewRoute, {
      authContext: authContext({ onAuthenticationFailure }),
    });
    await vi.waitFor(() => expect(byTestId(unauthorized, 'identity-load-error').textContent).toContain('HTTP 401'));
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function authContext(overrides: Partial<UnifiedAdminContext> = {}): UnifiedAdminContext {
  return {
    auth: { configured: true, authenticated: true, username: 'demo', accessToken: 'owner-token', claims: null },
    authConfig: null,
    accessTokenProvider: async () => 'owner-token',
    onAuthenticationFailure: vi.fn(),
    login: async () => {},
    logout: async () => {},
    ...overrides,
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
