import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { byTestId, cleanupRenderedComponents, renderComponent } from '$lib/test-utils/svelte-component-test';
import ExtensionCreateRoute from './ExtensionCreateRoute.svelte';

beforeEach(() => window.history.replaceState(null, '', 'http://localhost:3000/admin-new/extensions/new'));
afterEach(async () => { vi.unstubAllGlobals(); await cleanupRenderedComponents(); });

describe('ExtensionCreateRoute', () => {
  it('creates an extension and delegates detail navigation', async () => {
    const navigateToExtension = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse(extensionPayload(), 201));
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(ExtensionCreateRoute, { authContext: authContext(), navigateToExtension });
    await input(target, 'extension-create-name', 'Login helper');
    byTestId(target, 'extension-create-submit').click();

    await vi.waitFor(() => expect(navigateToExtension).toHaveBeenCalledWith(expect.objectContaining({ id: 'extension-1' })));
    expect(fetchImpl.mock.calls[0]?.[1]?.body).toBe(JSON.stringify({ name: 'Login helper', description: null, labels: {} }));
  });

  it('shows API validation failures', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => jsonResponse({ error: 'extension rejected' }, 400)));
    const target = renderComponent(ExtensionCreateRoute, { authContext: authContext() });
    await input(target, 'extension-create-name', 'Login helper');
    byTestId(target, 'extension-create-submit').click();
    await vi.waitFor(() => expect(byTestId(target, 'extension-create-error').textContent).toContain('extension rejected'));
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

function authContext(): UnifiedAdminContext {
  return { auth: { configured: true, authenticated: true, username: 'demo', accessToken: 'token', claims: null }, authConfig: null, accessTokenProvider: async () => 'shell-token', onAuthenticationFailure: vi.fn(), login: async () => {}, logout: async () => {} };
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), { status, headers: { 'content-type': 'application/json' } });
}

function extensionPayload() {
  return { id: 'extension-1', name: 'Login helper', description: null, enabled: true, latest_version: null, labels: {}, created_at: '2026-08-07T08:00:00.000Z', updated_at: '2026-08-07T09:00:00.000Z' };
}
