import { describe, expect, it, vi } from 'vitest';

import {
  SessionFileClient,
  SessionFileClientError,
  toSessionFileBindingResource,
  toSessionFileResource,
} from './session-file-client';

describe('SessionFileClient', () => {
  it('uses authenticated encoded session routes for files and bindings', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (input, init) => {
      const url = String(input);
      expect(new Headers(init?.headers).get('authorization')).toBe('Bearer owner-token');
      if (url.endsWith('/files')) {
        return jsonResponse({ files: [filePayload()] });
      }
      if (url.endsWith('/file-bindings')) {
        return jsonResponse({ bindings: [bindingPayload()] });
      }
      return new Response('not found', { status: 404 });
    });
    const client = createClient(fetchImpl);

    expect((await client.listSessionFiles('session/one')).files[0]?.name).toBe('report.pdf');
    expect((await client.listSessionFileBindings('session/one')).bindings[0]?.mount_path)
      .toBe('inputs/report.pdf');
    expect(fetchImpl.mock.calls.map((call) => String(call[0]))).toEqual([
      'http://localhost:8080/api/v1/sessions/session%2Fone/files',
      'http://localhost:8080/api/v1/sessions/session%2Fone/file-bindings',
    ]);
  });

  it('creates and removes bindings with encoded ids and exact payloads', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (_input, init) => jsonResponse(bindingPayload({
      state: init?.method === 'DELETE' ? 'removed' : 'pending',
    })));
    const client = createClient(fetchImpl);
    const request = {
      workspace_id: 'workspace-1',
      file_id: 'file-1',
      mount_path: 'inputs/report.pdf',
      mode: 'read_only' as const,
      labels: { source: 'admin-new' },
    };

    await client.createSessionFileBinding('session/one', request);
    await client.removeSessionFileBinding('session/one', 'binding/one');

    expect(String(fetchImpl.mock.calls[0]?.[0])).toContain('/session%2Fone/file-bindings');
    expect(fetchImpl.mock.calls[0]?.[1]?.method).toBe('POST');
    expect(JSON.parse(String(fetchImpl.mock.calls[0]?.[1]?.body))).toEqual(request);
    expect(String(fetchImpl.mock.calls[1]?.[0])).toContain('/file-bindings/binding%2Fone');
    expect(fetchImpl.mock.calls[1]?.[1]?.method).toBe('DELETE');
  });

  it('downloads only through API-provided same-origin content paths', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response('exact bytes', {
      status: 200,
      headers: { 'content-type': 'application/pdf' },
    }));
    const client = createClient(fetchImpl);

    expect(await (await client.downloadSessionFile(toSessionFileResource(filePayload()))).text()).toBe('exact bytes');
    expect(await (await client.downloadSessionFileBinding(toSessionFileBindingResource(bindingPayload()))).text())
      .toBe('exact bytes');
    expect(fetchImpl.mock.calls.map((call) => String(call[0]))).toEqual([
      'http://localhost:8080/api/v1/sessions/session-1/files/file-1/content',
      'http://localhost:8080/api/v1/sessions/session-1/file-bindings/binding-1/content',
    ]);
  });

  it('rejects invalid enum, labels, byte counts, and provenance payloads', () => {
    expect(() => toSessionFileResource(filePayload({ source: 'filesystem' })))
      .toThrow(SessionFileClientError);
    expect(() => toSessionFileResource(filePayload({ byte_count: -1 })))
      .toThrow('non-negative');
    expect(() => toSessionFileResource(filePayload({ labels: { owner: 7 } })))
      .toThrow('labels.owner');
    expect(() => toSessionFileBindingResource(bindingPayload({ mode: 'execute' })))
      .toThrow('unsupported');
    expect(() => toSessionFileBindingResource(bindingPayload({ provenance: [] })))
      .toThrow('must be an object');
  });

  it('delegates expired authentication to the global handler', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new SessionFileClient({
      baseUrl: 'http://localhost:8080',
      accessTokenProvider: async () => 'expired',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listSessionFiles('session-1')).rejects.toThrow('HTTP 401');
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
  });
});

function createClient(fetchImpl: typeof fetch): SessionFileClient {
  return new SessionFileClient({
    baseUrl: 'http://localhost:8080',
    accessTokenProvider: async () => 'owner-token',
    fetchImpl,
  });
}

function filePayload(overrides: Readonly<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    id: 'file-1',
    session_id: 'session-1',
    name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234567890abcdef',
    source: 'browser_download',
    labels: { team: 'support' },
    content_path: '/api/v1/sessions/session-1/files/file-1/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
    ...overrides,
  };
}

function bindingPayload(overrides: Readonly<Record<string, unknown>> = {}): Record<string, unknown> {
  return {
    id: 'binding-1',
    session_id: 'session-1',
    workspace_id: 'workspace-1',
    file_id: 'file-1',
    file_name: 'report.pdf',
    media_type: 'application/pdf',
    byte_count: 2048,
    sha256_hex: '1234567890abcdef1234567890abcdef',
    provenance: { source: 'case-system' },
    mount_path: 'inputs/report.pdf',
    mode: 'read_only',
    state: 'materialized',
    error: null,
    labels: { source: 'admin-new' },
    content_path: '/api/v1/sessions/session-1/file-bindings/binding-1/content',
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
    ...overrides,
  };
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
