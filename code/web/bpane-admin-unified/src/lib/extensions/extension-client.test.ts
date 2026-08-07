import { describe, expect, it, vi } from 'vitest';

import {
  ExtensionCatalogClient,
  ExtensionCatalogError,
  toExtensionDefinitionListResponse,
  toExtensionVersionResource,
} from './extension-client';

describe('ExtensionCatalogClient', () => {
  it('loads the extension catalog through authenticated requests', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({ extensions: [extensionPayload()] }, 200));
    const client = catalogClient(fetchImpl);

    const response = await client.listExtensions();

    expect(response.extensions[0]).toMatchObject({ id: 'extension-1', name: 'Password helper' });
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
    expect(fetchImpl.mock.calls[0]?.[0]).toEqual(new URL('http://browserpane.test/api/v1/extensions'));
  });

  it('creates, reads, versions, disables, and enables extensions', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/api/v1/extensions') && init?.method === 'POST') {
        return jsonResponse(extensionPayload(), 201);
      }
      if (url.endsWith('/api/v1/extensions/extension-1/versions')) {
        return jsonResponse(versionPayload(), 201);
      }
      if (url.endsWith('/disable')) {
        return jsonResponse(extensionPayload({ enabled: false }), 200);
      }
      if (url.endsWith('/enable')) {
        return jsonResponse(extensionPayload({ enabled: true }), 200);
      }
      return jsonResponse(extensionPayload(), 200);
    });
    const client = catalogClient(fetchImpl);

    await client.createExtension({ name: 'Password helper', labels: { team: 'platform' } });
    await client.getExtension('extension-1');
    const version = await client.createExtensionVersion('extension-1', {
      version: '1.0.0',
      install_path: '/opt/browserpane/extensions/password-helper',
    });
    const disabled = await client.setExtensionEnabled('extension-1', false);
    const enabled = await client.setExtensionEnabled('extension-1', true);

    expect(version).toMatchObject({ id: 'version-1', version: '1.0.0' });
    expect(disabled.enabled).toBe(false);
    expect(enabled.enabled).toBe(true);
    expect(fetchImpl.mock.calls.map((call) => [String(call[0]), call[1]?.method])).toEqual([
      ['http://browserpane.test/api/v1/extensions', 'POST'],
      ['http://browserpane.test/api/v1/extensions/extension-1', 'GET'],
      ['http://browserpane.test/api/v1/extensions/extension-1/versions', 'POST'],
      ['http://browserpane.test/api/v1/extensions/extension-1/disable', 'POST'],
      ['http://browserpane.test/api/v1/extensions/extension-1/enable', 'POST'],
    ]);
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new ExtensionCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listExtensions()).rejects.toMatchObject({ status: 401 });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });

  it('rejects malformed resource payloads', () => {
    expect(() => toExtensionDefinitionListResponse({ extensions: {} })).toThrow(ExtensionCatalogError);
    expect(() => toExtensionDefinitionListResponse({ extensions: [{ ...extensionPayload(), enabled: 'yes' }] }))
      .toThrow('extension enabled must be a boolean');
    expect(() => toExtensionVersionResource({ ...versionPayload(), install_path: null }))
      .toThrow('extension version install_path must be a string');
  });
});

function catalogClient(fetchImpl: typeof fetch): ExtensionCatalogClient {
  return new ExtensionCatalogClient({
    baseUrl: 'http://browserpane.test',
    accessTokenProvider: () => 'token-1',
    fetchImpl,
  });
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function extensionPayload(overrides: Partial<{ readonly enabled: boolean }> = {}) {
  return {
    id: 'extension-1',
    name: 'Password helper',
    description: 'Approved login helper',
    enabled: overrides.enabled ?? true,
    latest_version: '1.0.0',
    labels: { team: 'platform' },
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}

function versionPayload() {
  return {
    id: 'version-1',
    extension_definition_id: 'extension-1',
    version: '1.0.0',
    install_path: '/opt/browserpane/extensions/password-helper',
    created_at: '2026-08-07T09:00:00.000Z',
  };
}
