import { describe, expect, it, vi } from 'vitest';

import {
  BrowserContextCatalogClient,
  BrowserContextCatalogError,
  toBrowserContextListResponse,
} from './browser-context-client';

describe('BrowserContextCatalogClient', () => {
  it('loads browser contexts through the authenticated control API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify({ contexts: [browserContextPayload()] }), { status: 200 }));
    const client = new BrowserContextCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listBrowserContexts();

    expect(response.contexts[0]).toMatchObject({
      id: 'context-1',
      name: 'Support baseline',
      project_id: 'project-1',
      persistence_mode: 'reusable',
      state: 'ready',
      usage: {
        visible_session_count: 1,
        active_runtime_session_count: 0,
      },
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/browser-contexts'),
      expect.objectContaining({ method: 'GET' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new BrowserContextCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listBrowserContexts()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('loads, creates, and deletes browser contexts', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify(browserContextPayload()), { status: 200 }));
    const client = new BrowserContextCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });
    const request = {
      project_id: null,
      name: 'Owner baseline',
      description: null,
      labels: {},
      persistence_mode: 'reusable' as const,
      retention_sec: 604800,
      max_profile_storage_bytes: null,
    };

    await client.getBrowserContext('context-1');
    await client.createBrowserContext(request);
    await client.deleteBrowserContext('context-1');

    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/browser-contexts/context-1'));
    expect(fetchImpl.mock.calls[0]?.[1]?.method).toBe('GET');
    expect(fetchImpl.mock.calls[1]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/browser-contexts'));
    expect(fetchImpl.mock.calls[1]?.[1]?.method).toBe('POST');
    expect(fetchImpl.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(request));
    expect(fetchImpl.mock.calls[2]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/browser-contexts/context-1'));
    expect(fetchImpl.mock.calls[2]?.[1]?.method).toBe('DELETE');
  });

  it('loads project binding options from the project catalog', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify({
        projects: [{ id: 'project-1', name: 'Support', state: 'active', ignored: true }],
      }), { status: 200 }));
    const client = new BrowserContextCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listProjectOptions();

    expect(response.projects).toEqual([{ id: 'project-1', name: 'Support', state: 'active' }]);
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/projects'));
  });

  it('defaults missing usage payloads defensively', () => {
    const response = toBrowserContextListResponse({
      contexts: [{
        id: 'context-1',
        name: 'Support baseline',
        labels: {},
        persistence_mode: 'reusable',
        state: 'ready',
        created_at: '2026-06-18T10:00:00.000Z',
        updated_at: '2026-06-18T10:00:00.000Z',
      }],
    });

    expect(response.contexts[0]?.usage).toEqual({
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: null,
      profile_storage_limit_exceeded: false,
    });
  });

  it('rejects invalid list payloads', () => {
    expect(() => toBrowserContextListResponse({ contexts: {} })).toThrow(BrowserContextCatalogError);
  });
});

function browserContextPayload() {
  return {
    id: 'context-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support baseline',
    description: 'Reusable support profile',
    labels: { team: 'support' },
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: '2026-06-25T10:00:00.000Z',
    max_profile_storage_bytes: 1073741824,
    state: 'ready',
    usage: {
      visible_session_count: 1,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: 1048576,
      profile_storage_limit_exceeded: false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: '2026-06-18T10:00:00.000Z',
    deleted_at: null,
  };
}
