import { describe, expect, it, vi } from 'vitest';
import {
  AdminApiRequestError,
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  parseAdminApiErrorBody,
  type AdminApiRequestFailure,
  type FetchLike,
} from './authenticated-api';

describe('AuthenticatedApiClient', () => {
  it('adds the bearer token and preserves request headers and bodies', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => jsonResponse({ ok: true }, 201));
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test/admin-new/',
      accessTokenProvider: async () => 'token-1',
      fetchImpl,
    });

    const response = await client.request('/api/v1/projects', {
      method: 'POST',
      headers: { 'content-type': 'application/json', 'x-request-id': 'request-1' },
      body: JSON.stringify({ name: 'Support' }),
    });

    expect(response.status).toBe(201);
    const [input, init] = fetchImpl.mock.calls[0]!;
    const headers = new Headers(init?.headers);
    expect(input).toEqual(new URL('https://browserpane.test/api/v1/projects'));
    expect(init?.method).toBe('POST');
    expect(init?.body).toBe(JSON.stringify({ name: 'Support' }));
    expect(headers.get('authorization')).toBe('Bearer token-1');
    expect(headers.get('content-type')).toBe('application/json');
    expect(headers.get('x-request-id')).toBe('request-1');
  });

  it('returns native empty and binary responses unchanged', async () => {
    const responses = [
      new Response(null, { status: 204 }),
      new Response(new Uint8Array([1, 2, 3]), { status: 200, headers: { 'content-type': 'application/octet-stream' } }),
    ];
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl: async () => responses.shift()!,
    });

    expect((await client.request('/empty')).status).toBe(204);
    expect([...new Uint8Array(await (await client.request('/binary')).arrayBuffer())]).toEqual([1, 2, 3]);
  });

  it('supports optional authentication without adding an empty bearer header', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => new Response(null, { status: 204 }));
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      authentication: 'optional',
      fetchImpl,
    });

    await client.request('/health');

    expect(new Headers(fetchImpl.mock.calls[0]?.[1]?.headers).has('authorization')).toBe(false);
  });

  it('rejects missing required tokens before fetch', async () => {
    const fetchImpl = vi.fn<FetchLike>();
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => null,
      fetchImpl,
    });

    await expect(client.request('/api/v1/projects')).rejects.toMatchObject({
      code: 'missing_token',
      status: null,
      message: 'No active admin access token is available.',
    });
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('retains safe structured error metadata and reports 401 once', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => jsonResponse({
        error: 'Access token expired.',
        code: 'token_expired',
        category: 'authentication',
        recovery_hint: 'Sign in again.',
        internal_trace: 'must-not-be-retained',
      }, 401),
      onAuthenticationFailure,
    });

    await expect(client.request('/api/v1/projects')).rejects.toMatchObject({
      status: 401,
      code: 'http_error',
      apiMessage: 'Access token expired.',
      apiCode: 'token_expired',
      apiCategory: 'authentication',
      recoveryHint: 'Sign in again.',
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();
    expect(onAuthenticationFailure.mock.calls[0]?.[0]).not.toHaveProperty('internal_trace');
  });

  it('keeps the original request failure when auth recovery throws', async () => {
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'expired-token',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure: () => {
        throw new Error('recovery failed');
      },
    });

    await expect(client.request('/api/v1/projects')).rejects.toMatchObject({
      status: 401,
      apiMessage: 'unauthorized',
    });
  });

  it('normalizes network and abort failures through a custom domain factory', async () => {
    const failures: AdminApiRequestFailure[] = [];
    const errors = [new TypeError('network down'), new DOMException('cancelled', 'AbortError')];
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl: async () => {
        throw errors.shift();
      },
      errorFactory: (failure) => {
        failures.push(failure);
        return new Error(formatAdminApiRequestError('Project catalog request', failure));
      },
    });

    await expect(client.request('/network')).rejects.toThrow('gateway could not be reached');
    await expect(client.request('/aborted')).rejects.toThrow('was cancelled');
    expect(failures.map((failure) => failure.code)).toEqual(['network_error', 'request_aborted']);
  });

  it('uses a bounded plain-text fallback for non-JSON errors', async () => {
    const body = `denied-${'x'.repeat(20_000)}`;
    const client = new AuthenticatedApiClient({
      baseUrl: 'https://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl: async () => new Response(body, { status: 503 }),
    });

    const error = await client.request('/api/v1/projects').catch((value: unknown) => value);
    expect(error).toBeInstanceOf(AdminApiRequestError);
    expect((error as AdminApiRequestError).apiMessage.length).toBe(16_384);
    expect((error as AdminApiRequestError).message).not.toContain('authorization');
  });
});

describe('admin API error parsing', () => {
  it('parses documented gateway fields and ignores non-string metadata', () => {
    expect(parseAdminApiErrorBody(JSON.stringify({
      error: 'Conflict.',
      code: 'resource_conflict',
      category: 'validation',
      recovery_hint: 'Refresh and retry.',
      extra: { secret: true },
    }))).toEqual({
      message: 'Conflict.',
      code: 'resource_conflict',
      category: 'validation',
      recoveryHint: 'Refresh and retry.',
    });
    expect(parseAdminApiErrorBody(JSON.stringify({ error: 42, code: false }))).toEqual({ message: '' });
  });

  it('supports empty, plain-text, and scalar JSON error bodies', () => {
    expect(parseAdminApiErrorBody('')).toEqual({ message: '' });
    expect(parseAdminApiErrorBody('denied')).toEqual({ message: 'denied' });
    expect(parseAdminApiErrorBody('"denied"')).toEqual({ message: '"denied"' });
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}
