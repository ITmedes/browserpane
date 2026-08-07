import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import ExtensionDetailRoute from './ExtensionDetailRoute.svelte';

beforeEach(() =>
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/extensions/extension-1'),
);
afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('ExtensionDetailRoute', () => {
  it('loads, versions, and changes extension state', async () => {
    let enabled = true;
    let latestVersion: string | null = null;
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/versions')) {
        latestVersion = '1.0.0';
        return jsonResponse(
          {
            id: 'version-1',
            extension_definition_id: 'extension-1',
            version: '1.0.0',
            install_path: '/opt/extensions/login',
            created_at: '2026-08-07T09:30:00.000Z',
          },
          201,
        );
      }
      if (url.endsWith('/disable')) {
        enabled = false;
        return jsonResponse(extensionPayload(enabled, latestVersion), 200);
      }
      if (init?.method === 'GET')
        return jsonResponse(extensionPayload(enabled, latestVersion), 200);
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(ExtensionDetailRoute, { authContext: authContext() });
    await vi.waitFor(() =>
      expect(byTestId(target, 'extension-detail-name').textContent).toContain('Login helper'),
    );

    await input(target, 'extension-version-value', '1.0.0');
    await input(target, 'extension-version-path', '/opt/extensions/login');
    byTestId(target, 'extension-version-submit').click();
    await vi.waitFor(() =>
      expect(byTestId(target, 'extension-action-success').textContent).toContain('published'),
    );
    expect(byTestId(target, 'extension-detail-version').textContent).toContain('1.0.0');

    byTestId(target, 'extension-toggle-enabled').click();
    await vi.waitFor(() =>
      expect(byTestId(target, 'extension-detail-state').textContent).toContain('Disabled'),
    );
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
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

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function extensionPayload(enabled: boolean, latestVersion: string | null) {
  return {
    id: 'extension-1',
    name: 'Login helper',
    description: null,
    enabled,
    latest_version: latestVersion,
    labels: {},
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
