// @ts-nocheck
import { describe, expect, it } from 'vitest';

import { EXIT_CODES, runBpaneCli } from '../../scripts/bpane-cli.mjs';

function createIo() {
  let stdout = '';
  let stderr = '';
  return {
    io: {
      stdout: { write: (value: string) => { stdout += value; } },
      stderr: { write: (value: string) => { stderr += value; } },
    },
    stdout: () => stdout,
    stderr: () => stderr,
  };
}

function jsonResponse(body: unknown, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    text: async () => body === null || body === undefined ? '' : JSON.stringify(body),
  };
}

function createFetch(...responses: unknown[]) {
  const calls: Array<{ url: string; init: Record<string, unknown> }> = [];
  const fetchImpl = async (url: string, init: Record<string, unknown> = {}) => {
    calls.push({ url, init });
    const next = responses.shift();
    if (next instanceof Error) {
      throw next;
    }
    return next ?? jsonResponse({ ok: true });
  };
  return { calls, fetchImpl };
}

function parseStdout(io: ReturnType<typeof createIo>) {
  return JSON.parse(io.stdout());
}

function parseStderr(io: ReturnType<typeof createIo>) {
  return JSON.parse(io.stderr());
}

describe('bpane operator CLI', () => {
  it('lists sessions through the owner-scoped API', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ sessions: [{ id: 'session-1' }] }));

    const code = await runBpaneCli(
      ['session', 'list', '--base-url', 'http://bpane.example/root/'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ sessions: [{ id: 'session-1' }] });
    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe('http://bpane.example/api/v1/sessions');
    expect(calls[0].init.headers.Authorization).toBe('Bearer token-1');
  });

  it('requires a bearer token for session commands', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ sessions: [] }));

    const code = await runBpaneCli(['session', 'list'], {}, io.io, fetchImpl);

    expect(code).toBe(EXIT_CODES.auth);
    expect(calls).toHaveLength(0);
    expect(parseStderr(io).code).toBe('AUTH_REQUIRED');
  });

  it('fetches a session status by id', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ state: 'running' }));

    const code = await runBpaneCli(
      ['session', 'status', 'session/with space'],
      { BPANE_BASE_URL: 'http://localhost:8080', BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ state: 'running' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/sessions/session%2Fwith%20space/status');
  });

  it('derives MCP health from the configured bridge control URL with path prefixes', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        mcpBridge: {
          controlUrl: 'http://mcp.example/prefix/control-session',
          clientId: 'bpane-mcp-bridge',
        },
      }),
      jsonResponse({ status: 'ok' }),
    );

    const code = await runBpaneCli(
      ['mcp', 'health', '--base-url', 'http://bpane.example'],
      {},
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ status: 'ok' });
    expect(calls.map((call) => call.url)).toEqual([
      'http://bpane.example/auth-config.json',
      'http://mcp.example/prefix/health',
    ]);
  });

  it('authorizes a session for the configured MCP bridge client', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ id: 'session-1' }));

    const code = await runBpaneCli(
      ['mcp', 'authorize', 'session-1'],
      {
        BPANE_BASE_URL: 'http://localhost:8080',
        BPANE_ACCESS_TOKEN: 'token-1',
        BPANE_MCP_CLIENT_ID: 'bridge-client',
        BPANE_MCP_ISSUER: 'issuer-1',
        BPANE_MCP_DISPLAY_NAME: 'Bridge Display',
      },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ id: 'session-1' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/sessions/session-1/automation-owner');
    expect(calls[0].init.method).toBe('POST');
    expect(calls[0].init.headers.Authorization).toBe('Bearer token-1');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      client_id: 'bridge-client',
      issuer: 'issuer-1',
      display_name: 'Bridge Display',
    });
  });

  it('sets and clears the MCP bridge default session', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ session: { id: 'session-1' } }),
      jsonResponse({ ok: true }),
    );

    const env = {
      BPANE_MCP_CONTROL_URL: 'http://localhost:8931/control-session',
    };
    const setCode = await runBpaneCli(['mcp', 'set-default', 'session-1'], env, io.io, fetchImpl);
    const clearCode = await runBpaneCli(['mcp', 'clear-default'], env, io.io, fetchImpl);

    expect(setCode).toBe(EXIT_CODES.ok);
    expect(clearCode).toBe(EXIT_CODES.ok);
    expect(calls[0]).toMatchObject({
      url: 'http://localhost:8931/control-session',
      init: { method: 'PUT' },
    });
    expect(JSON.parse(calls[0].init.body)).toEqual({ session_id: 'session-1' });
    expect(calls[1]).toMatchObject({
      url: 'http://localhost:8931/control-session',
      init: { method: 'DELETE' },
    });
  });

  it('maps HTTP failures to a stable JSON error and exit code', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ error: 'session not found' }, 404));

    const code = await runBpaneCli(
      ['session', 'get', 'missing-session'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.api);
    expect(calls).toHaveLength(1);
    expect(parseStderr(io)).toMatchObject({
      ok: false,
      code: 'HTTP_ERROR',
      status: 404,
      body: { error: 'session not found' },
    });
  });
});
