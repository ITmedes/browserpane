import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import CredentialBindingCreateRoute from './CredentialBindingCreateRoute.svelte';
beforeEach(() =>
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/credential-bindings/new'),
);
afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});
describe('CredentialBindingCreateRoute', () => {
  it('provisions through Vault and removes the write-only value by navigating', async () => {
    const navigateToBinding = vi.fn();
    const submittedSecret = 'one-time-secret';
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) =>
      String(input).endsWith('/api/v1/projects')
        ? jsonResponse({ projects: [] }, 200)
        : jsonResponse(bindingPayload(), init?.method === 'POST' ? 201 : 200),
    );
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(CredentialBindingCreateRoute, {
      authContext: authContext(),
      navigateToBinding,
    });
    await input(target, 'credential-binding-name', 'Support login');
    await input(target, 'credential-binding-secret-payload', `{"password":"${submittedSecret}"}`);
    byTestId(target, 'credential-binding-create-submit').click();
    await vi.waitFor(() =>
      expect(navigateToBinding).toHaveBeenCalledWith(expect.objectContaining({ id: 'binding-1' })),
    );
    const createCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/credential-bindings'),
    );
    expect(createCall?.[1]?.body).toContain(submittedSecret);
    expect(JSON.stringify(navigateToBinding.mock.calls[0]?.[0])).not.toContain(submittedSecret);
  });
  it('keeps API errors visible without echoing a submitted secret', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(async (input) =>
        String(input).endsWith('/api/v1/projects')
          ? jsonResponse({ projects: [] }, 200)
          : jsonResponse({ error: 'Vault unavailable' }, 503),
      ),
    );
    const target = renderComponent(CredentialBindingCreateRoute, { authContext: authContext() });
    await input(target, 'credential-binding-name', 'Support login');
    await input(target, 'credential-binding-secret-payload', '{"password":"sensitive-value"}');
    byTestId(target, 'credential-binding-create-submit').click();
    await vi.waitFor(() =>
      expect(byTestId(target, 'credential-binding-create-error').textContent).toContain(
        'Vault unavailable',
      ),
    );
    expect(byTestId(target, 'credential-binding-create-error').textContent).not.toContain(
      'sensitive-value',
    );
  });
});
async function input(target: HTMLElement, testId: string, value: string) {
  const element = byTestId(target, testId) as HTMLInputElement | HTMLTextAreaElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}
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
