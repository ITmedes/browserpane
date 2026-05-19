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

  it('creates a session with structured CLI options', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ id: 'session-1' }));

    const code = await runBpaneCli(
      [
        'session',
        'create',
        '--label',
        'suite=cli',
        '--template-id',
        'desktop',
        '--owner-mode',
        'collaborative',
        '--width',
        '1440',
        '--height',
        '900',
        '--idle-timeout-sec',
        '120',
        '--integration-json',
        '{"ticket":"abc"}',
        '--extension-id',
        '018f1a5b-0784-71bf-ae46-0c973f00aa11',
        '--recording-mode',
        'manual',
        '--recording-retention-sec',
        '3600',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ id: 'session-1' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/sessions');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      template_id: 'desktop',
      owner_mode: 'collaborative',
      viewport: { width: 1440, height: 900 },
      idle_timeout_sec: 120,
      labels: { suite: 'cli' },
      integration_context: { ticket: 'abc' },
      extension_ids: ['018f1a5b-0784-71bf-ae46-0c973f00aa11'],
      recording: { mode: 'manual', format: 'webm', retention_sec: 3600 },
    });
  });

  it('mints access, automation access, and disconnects all session clients', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ token_type: 'session_connect_ticket' }),
      jsonResponse({ token_type: 'session_automation_access_token' }),
      jsonResponse({ state: 'idle' }),
    );

    const env = { BPANE_ACCESS_TOKEN: 'token-1' };
    const accessCode = await runBpaneCli(['session', 'access-token', 'session-1'], env, io.io, fetchImpl);
    const automationCode = await runBpaneCli(['session', 'automation-access', 'session-1'], env, io.io, fetchImpl);
    const disconnectCode = await runBpaneCli(['session', 'disconnect-all', 'session-1'], env, io.io, fetchImpl);

    expect(accessCode).toBe(EXIT_CODES.ok);
    expect(automationCode).toBe(EXIT_CODES.ok);
    expect(disconnectCode).toBe(EXIT_CODES.ok);
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/sessions/session-1/access-tokens', 'POST'],
      ['http://localhost:8080/api/v1/sessions/session-1/automation-access', 'POST'],
      ['http://localhost:8080/api/v1/sessions/session-1/connections/disconnect-all', 'POST'],
    ]);
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

  it('diagnoses MCP delegation mismatches with remediation hints', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        mcpBridge: {
          controlUrl: 'http://mcp.example/control-session',
          clientId: 'bridge-client',
        },
      }),
      jsonResponse({ status: 'ok', managed_sessions: [] }),
      jsonResponse({ session: { id: 'other-session' } }),
      jsonResponse({
        id: 'session-1',
        state: 'running',
        automation_delegate: { client_id: 'other-client' },
      }),
      jsonResponse({ mcp_owner: false }),
    );

    const code = await runBpaneCli(
      ['mcp', 'doctor', 'session-1', '--base-url', 'http://bpane.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    const output = parseStdout(io);
    expect(output.ok).toBe(false);
    expect(output.issues.map((issue) => issue.code)).toEqual([
      'MCP_DELEGATE_MISMATCH',
      'MCP_DEFAULT_SESSION_MISMATCH',
    ]);
    expect(calls.map((call) => call.url)).toEqual([
      'http://bpane.example/auth-config.json',
      'http://mcp.example/health',
      'http://mcp.example/control-session',
      'http://bpane.example/api/v1/sessions/session-1',
      'http://bpane.example/api/v1/sessions/session-1/status',
    ]);
  });

  it('fails MCP preflight when diagnostics find issues', async () => {
    const io = createIo();
    const { fetchImpl } = createFetch(
      jsonResponse({
        mcpBridge: {
          controlUrl: 'http://mcp.example/control-session',
          clientId: 'bridge-client',
        },
      }),
      jsonResponse({ status: 'ok', managed_sessions: [] }),
      jsonResponse({ session: null }),
      jsonResponse({
        id: 'session-1',
        state: 'running',
        automation_delegate: null,
      }),
      jsonResponse({ mcp_owner: false }),
    );

    const code = await runBpaneCli(
      ['mcp', 'preflight', 'session-1', '--base-url', 'http://bpane.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.api);
    const output = parseStdout(io);
    expect(output.ok).toBe(false);
    expect(output.issues.map((issue) => issue.code)).toEqual([
      'MCP_DELEGATE_MISSING',
      'MCP_DEFAULT_SESSION_MISMATCH',
    ]);
    expect(io.stderr()).toBe('');
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

  it('dry-runs bounded session cleanup by default', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({
      sessions: [
        {
          id: 'session-1',
          state: 'stopped',
          labels: { suite: 'cli' },
          automation_delegate: { client_id: 'bridge-client' },
          status: { connection_counts: { total_clients: 0 } },
          created_at: '2026-05-18T10:00:00Z',
          updated_at: '2026-05-18T10:00:00Z',
        },
        {
          id: 'session-2',
          state: 'running',
          labels: { suite: 'cli' },
          created_at: '2026-05-18T10:00:00Z',
        },
      ],
    }));

    const code = await runBpaneCli(
      ['session', 'cleanup', '--label', 'suite=cli'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      dry_run: true,
      candidate_count: 1,
      candidates: [{ id: 'session-1', state: 'stopped' }],
    });
    expect(calls).toHaveLength(1);
  });

  it('requires a bounding filter before confirmed cleanup', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ sessions: [] }));

    const code = await runBpaneCli(
      ['session', 'cleanup', '--confirm'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.usage);
    expect(calls).toHaveLength(0);
    expect(parseStderr(io).error).toContain('requires at least one bounding');
  });

  it('executes confirmed cleanup through revoke, disconnect, and kill operations', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        sessions: [
          {
            id: 'session-1',
            state: 'stopped',
            labels: { suite: 'cli' },
            automation_delegate: { client_id: 'bridge-client' },
            status: { connection_counts: { total_clients: 0 } },
            created_at: '2026-05-18T10:00:00Z',
          },
        ],
      }),
      jsonResponse({ id: 'session-1' }),
      jsonResponse({ state: 'stopped' }),
      jsonResponse({ id: 'session-1', state: 'stopped' }),
    );

    const code = await runBpaneCli(
      ['session', 'cleanup', '--label', 'suite=cli', '--confirm'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      dry_run: false,
      result_count: 1,
      results: [{ session: { id: 'session-1' } }],
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/sessions', undefined],
      ['http://localhost:8080/api/v1/sessions/session-1/automation-owner', 'DELETE'],
      ['http://localhost:8080/api/v1/sessions/session-1/connections/disconnect-all', 'POST'],
      ['http://localhost:8080/api/v1/sessions/session-1/kill', 'POST'],
    ]);
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
