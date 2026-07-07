import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
import {
  byTestId,
  cleanupRenderedComponents,
  renderComponent,
} from '$lib/test-utils/svelte-component-test';
import BrowserContextCreateRoute from './BrowserContextCreateRoute.svelte';

beforeEach(() => {
  window.history.replaceState(null, '', 'http://localhost:3000/admin-new/browser-contexts/new');
});

afterEach(async () => {
  vi.unstubAllGlobals();
  await cleanupRenderedComponents();
});

describe('BrowserContextCreateRoute', () => {
  it('loads project options and creates a browser context', async () => {
    const navigateToContext = vi.fn();
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/projects')) {
        return jsonResponse(projectListPayload(), 200);
      }
      if (url.endsWith('/api/v1/browser-contexts') && init?.method === 'POST') {
        return jsonResponse(browserContextPayload({ name: 'Created context' }), 201);
      }
      return new Response('not found', { status: 404 });
    });
    vi.stubGlobal('fetch', fetchImpl);
    const target = renderComponent(BrowserContextCreateRoute, {
      authContext: authContext({ accessTokenProvider: async () => 'shell-token' }),
      navigateToContext,
    });

    await vi.waitFor(() => {
      expect(byTestId(target, 'browser-context-edit-form')).toBeInstanceOf(HTMLElement);
    });
    await input(target, 'browser-context-edit-name', 'Created context');
    byTestId(target, 'browser-context-edit-save').click();

    await vi.waitFor(() => {
      expect(navigateToContext).toHaveBeenCalledWith(expect.objectContaining({ id: 'context-1' }));
    });
    const createCall = fetchImpl.mock.calls.find((call) =>
      String(call[0]).endsWith('/api/v1/browser-contexts') && call[1]?.method === 'POST');
    expect(createCall?.[1]?.body).toBe(JSON.stringify({
      project_id: null,
      name: 'Created context',
      description: null,
      labels: {},
      persistence_mode: 'reusable',
      retention_sec: 604800,
      max_profile_storage_bytes: null,
    }));
    const headers = createCall?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer shell-token');
  });
});

async function input(target: HTMLElement, testId: string, value: string): Promise<void> {
  const element = byTestId(target, testId) as HTMLInputElement;
  element.value = value;
  element.dispatchEvent(new Event('input', { bubbles: true }));
  await tick();
}

function authContext(
  overrides: Partial<Pick<UnifiedAdminContext, 'accessTokenProvider' | 'onAuthenticationFailure'>> = {},
): UnifiedAdminContext {
  return {
    auth: {
      configured: true,
      authenticated: true,
      username: 'demo',
      accessToken: 'token',
      claims: null,
    },
    authConfig: null,
    accessTokenProvider: overrides.accessTokenProvider ?? (async () => 'token'),
    onAuthenticationFailure: overrides.onAuthenticationFailure ?? vi.fn(),
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

function projectListPayload() {
  return {
    projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
  };
}

function browserContextPayload(overrides: Partial<{ readonly name: string }> = {}) {
  return {
    id: 'context-1',
    project_id: null,
    project: null,
    name: overrides.name ?? 'Support baseline',
    description: null,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: null,
    state: 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: null,
      profile_storage_limit_exceeded: false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
