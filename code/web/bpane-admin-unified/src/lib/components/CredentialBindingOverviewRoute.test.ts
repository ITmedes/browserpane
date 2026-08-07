import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import CredentialBindingOverviewRoute from './CredentialBindingOverviewRoute.svelte';
beforeEach(() =>
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/credential-bindings'),
);
afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});
describe('CredentialBindingOverviewRoute', () => {
  it('loads safe binding metadata through authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      jsonResponse({ credential_bindings: [bindingPayload()] }, 200),
    );
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(CredentialBindingOverviewRoute, { authContext: authContext() });
    await vi.waitFor(() =>
      expect(byTestId(target, 'credential-bindings-list').textContent).toContain('Support login'),
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
function bindingPayload() {
  return {
    id: 'binding-1',
    project_id: null,
    project: null,
    name: 'Support login',
    provider: 'vault_kv_v2',
    external_ref: 'secret/data/binding-1',
    namespace: null,
    allowed_origins: [],
    injection_mode: 'form_fill',
    totp: null,
    labels: {},
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
