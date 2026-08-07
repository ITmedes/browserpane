import { describe, expect, it, vi } from 'vitest';

import {
  BrowserContextCatalogClient,
  BrowserContextCatalogError,
  browserContextExportFilename,
  sanitizeBrowserContextArchiveFilename,
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

  it('clones, exports, and imports browser contexts through typed lifecycle requests', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url.endsWith('/export')) {
        return new Response('zip-bytes', {
          status: 200,
          headers: {
            'content-type': 'application/zip',
            'content-disposition': 'attachment; filename="support-baseline.zip"',
          },
        });
      }
      return new Response(JSON.stringify(browserContextPayload()), { status: 201 });
    });
    const client = new BrowserContextCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });
    const metadata = {
      project_id: 'project-1',
      name: 'Support copy',
      description: 'Copied support profile',
      labels: { team: 'support', source: 'import' },
      retention_sec: 86400,
      max_profile_storage_bytes: 2097152,
    };

    await client.cloneBrowserContext('context/one', metadata);
    const exported = await client.exportBrowserContext('context/one');
    const archive = new Blob(['archive'], { type: 'application/zip' });
    await client.importBrowserContext({ ...metadata, archive });

    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(
      new URL('http://browserpane.test/api/v1/browser-contexts/context%2Fone/clone'),
    );
    expect(fetchImpl.mock.calls[0]?.[1]).toMatchObject({
      method: 'POST',
      body: JSON.stringify(metadata),
    });
    expect(exported.filename).toBe('support-baseline.zip');
    expect(await exported.blob.text()).toBe('zip-bytes');
    const exportHeaders = fetchImpl.mock.calls[1]?.[1]?.headers as Headers;
    expect(exportHeaders.get('accept')).toBe('application/zip');
    const importHeaders = fetchImpl.mock.calls[2]?.[1]?.headers as Headers;
    expect(fetchImpl.mock.calls[2]?.[0]).toEqual(
      new URL('http://browserpane.test/api/v1/browser-contexts/import'),
    );
    expect(fetchImpl.mock.calls[2]?.[1]?.body).toBe(archive);
    expect(importHeaders.get('content-type')).toBe('application/zip');
    expect(importHeaders.get('x-bpane-browser-context-name')).toBe('Support copy');
    expect(importHeaders.get('x-bpane-browser-context-project-id')).toBe('project-1');
    expect(importHeaders.get('x-bpane-browser-context-description')).toBe('Copied support profile');
    expect(importHeaders.get('x-bpane-browser-context-labels')).toBe('{"team":"support","source":"import"}');
    expect(importHeaders.get('x-bpane-browser-context-retention-sec')).toBe('86400');
    expect(importHeaders.get('x-bpane-browser-context-max-profile-storage-bytes')).toBe('2097152');
  });

  it('omits nullable import metadata headers without changing the raw archive', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () =>
      new Response(JSON.stringify(browserContextPayload()), { status: 201 }));
    const client = new BrowserContextCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    await client.importBrowserContext({
      name: 'Minimal import',
      project_id: null,
      description: null,
      retention_sec: null,
      max_profile_storage_bytes: null,
      archive: new Blob(['archive']),
    });

    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.has('x-bpane-browser-context-project-id')).toBe(false);
    expect(headers.has('x-bpane-browser-context-description')).toBe(false);
    expect(headers.has('x-bpane-browser-context-retention-sec')).toBe(false);
    expect(headers.has('x-bpane-browser-context-max-profile-storage-bytes')).toBe(false);
  });

  it('derives safe export filenames from content disposition or the context id', () => {
    expect(browserContextExportFilename(
      new Headers({ 'content-disposition': "attachment; filename*=UTF-8''Support%20Profile.zip" }),
      'context-1',
    )).toBe('Support-Profile.zip');
    expect(browserContextExportFilename(
      new Headers({ 'content-disposition': 'attachment; filename="../../unsafe profile"' }),
      'context-1',
    )).toBe('unsafe-profile.zip');
    expect(browserContextExportFilename(new Headers(), 'context/one')).toBe(
      'browserpane-browser-context-context-one.zip',
    );
    expect(sanitizeBrowserContextArchiveFilename('...', 'context-1')).toBe(
      'browserpane-browser-context-context-1.zip',
    );
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
