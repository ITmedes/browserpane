import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import { sessionPayload } from '$lib/test-utils/session-fixtures';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import SessionPreviewRoute from './SessionPreviewRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/sessions/session-1/preview');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('SessionPreviewRoute', () => {
  it('loads the selected session and connects the popup viewport', async () => {
    const disconnect = vi.fn();
    const connect = vi.fn(async () => ({
      sessionId: 'session-1',
      gatewayUrl: 'https://localhost:4433/session/session-1',
      handle: { disconnect },
    }));
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/api/v1/sessions/session-1')) {
        return jsonResponse(sessionPayload(), 200);
      }
      return new Response('not found', { status: 404 });
    }));

    const target = renderComponent(SessionPreviewRoute, {
      authContext: authContext(),
      connectorFactory: () => ({ connect }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-status').textContent).toContain('Connected');
    });
    expect(byTestId(target, 'session-preview-title').textContent).toContain('session-1');
    expect(connect).toHaveBeenCalledOnce();

    byTestId(target, 'session-preview-disconnect').click();
    expect(disconnect).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-status').textContent).toContain('Disconnected');
    });
  });

  it('shows connection failures without embedding a broken canvas', async () => {
    vi.stubGlobal('fetch', vi.fn<typeof fetch>(async () => jsonResponse(sessionPayload(), 200)));
    const target = renderComponent(SessionPreviewRoute, {
      authContext: authContext(),
      connectorFactory: () => ({
        connect: async () => {
          throw new Error('preview failed');
        },
      }),
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'session-preview-error').textContent).toContain('preview failed');
    });
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
    accessTokenProvider: async () => 'token',
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
