import { describe, expect, it, vi } from 'vitest';

import {
  CredentialBindingCatalogClient,
  CredentialBindingCatalogError,
  toCredentialBindingListResponse,
  toCredentialBindingResource,
} from './credential-binding-client';

describe('CredentialBindingCatalogClient', () => {
  it('lists, creates, and reads bindings through authenticated requests', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      if (String(input).endsWith('/api/v1/credential-bindings') && init?.method === 'GET') {
        return jsonResponse({ credential_bindings: [bindingPayload()] }, 200);
      }
      return jsonResponse(bindingPayload(), init?.method === 'POST' ? 201 : 200);
    });
    const client = catalogClient(fetchImpl);
    const request = {
      name: 'Support login',
      provider: 'vault_kv_v2' as const,
      allowed_origins: ['https://support.example'],
      injection_mode: 'form_fill' as const,
      secret_payload: { fields: [{ selector: '#user', value: 'demo' }] },
      labels: { team: 'support' },
    };

    await client.listCredentialBindings();
    const created = await client.createCredentialBinding(request);
    await client.getCredentialBinding('binding-1');

    expect(created).toMatchObject({ id: 'binding-1', injection_mode: 'form_fill' });
    expect(JSON.stringify(created)).not.toContain('secret_payload');
    expect(fetchImpl.mock.calls[1]?.[1]?.body).toBe(JSON.stringify(request));
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('loads active and archived project options', async () => {
    const client = catalogClient(vi.fn<typeof fetch>(async () => jsonResponse({
      projects: [{ id: 'project-1', name: 'Support', state: 'active', ignored: true }],
    }, 200)));
    expect(await client.listProjectOptions()).toEqual({
      projects: [{ id: 'project-1', name: 'Support', state: 'active' }],
    });
  });

  it('ignores unexpected secret fields in responses and rejects malformed metadata', () => {
    const resource = toCredentialBindingResource({ ...bindingPayload(), secret_payload: { password: 'must-not-leak' } });
    expect(JSON.stringify(resource)).not.toContain('must-not-leak');
    expect(() => toCredentialBindingListResponse({ credential_bindings: {} })).toThrow(CredentialBindingCatalogError);
    expect(() => toCredentialBindingResource({ ...bindingPayload(), injection_mode: 'unknown' })).toThrow('must be one of');
  });

  it('delegates authentication failures', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new CredentialBindingCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });
    await expect(client.listCredentialBindings()).rejects.toMatchObject({ status: 401 });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function catalogClient(fetchImpl: typeof fetch): CredentialBindingCatalogClient {
  return new CredentialBindingCatalogClient({ baseUrl: 'http://browserpane.test', accessTokenProvider: () => 'token-1', fetchImpl });
}

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), { status, headers: { 'content-type': 'application/json' } });
}

function bindingPayload() {
  return {
    id: 'binding-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support login',
    provider: 'vault_kv_v2',
    external_ref: 'secret/data/bpane/credential-bindings/binding-1',
    namespace: 'support',
    allowed_origins: ['https://support.example'],
    injection_mode: 'form_fill',
    totp: null,
    labels: { team: 'support' },
    created_at: '2026-08-07T08:00:00.000Z',
    updated_at: '2026-08-07T09:00:00.000Z',
  };
}
