import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import CredentialBindingDetailRoute from './CredentialBindingDetailRoute.svelte';
beforeEach(() =>
  window.history.replaceState(
    null,
    '',
    'http://localhost:3000/admin-new/credential-bindings/binding-1',
  ),
);
afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});
describe('CredentialBindingDetailRoute', () => {
  it('loads and refreshes safe metadata without rendering unexpected secret fields', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      jsonResponse({ ...bindingPayload(), secret_payload: { password: 'must-not-render' } }, 200),
    );
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(CredentialBindingDetailRoute, { authContext: authContext() });
    await vi.waitFor(() =>
      expect(byTestId(target, 'credential-binding-detail-name').textContent).toContain(
        'Support login',
      ),
    );
    expect(target.textContent).not.toContain('must-not-render');
    byTestId(target, 'credential-binding-refresh-detail').click();
    await vi.waitFor(() =>
      expect(byTestId(target, 'credential-binding-action-success').textContent).toContain(
        'refreshed',
      ),
    );
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
