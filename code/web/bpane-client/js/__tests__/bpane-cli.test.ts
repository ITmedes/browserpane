// @ts-nocheck
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

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
    headers: new Headers({ 'content-type': 'application/json' }),
    text: async () => body === null || body === undefined ? '' : JSON.stringify(body),
  };
}

function binaryResponse(body: Buffer | string, contentType = 'application/octet-stream', status = 200) {
  const bytes = Buffer.isBuffer(body) ? body : Buffer.from(body);
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: new Headers({ 'content-type': contentType }),
    arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
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

async function withConfig(config: unknown, fn: (filePath: string) => Promise<void>) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-cli-test-'));
  const filePath = path.join(dir, 'config.json');
  try {
    await fs.writeFile(filePath, JSON.stringify(config), 'utf8');
    await fn(filePath);
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

describe('bpane operator CLI', () => {
  it('initializes a local CLI profile without persisting tokens by default', async () => {
    const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-cli-test-'));
    const filePath = path.join(dir, 'nested', 'config.json');
    try {
      const io = createIo();
      const code = await runBpaneCli(
        [
          'profile',
          'init',
          'local',
          '--config',
          filePath,
          '--base-url',
          'http://localhost:8080/admin/',
          '--mcp-control-url',
          'http://localhost:8931/control-session',
          '--mcp-client-id',
          'bpane-mcp-bridge',
          '--set-default',
        ],
        { BPANE_ACCESS_TOKEN: 'env-token' },
        io.io,
        async () => {
          throw new Error('profile init must not fetch');
        },
      );

      expect(code).toBe(EXIT_CODES.ok);
      expect(parseStdout(io)).toMatchObject({
        config_path: filePath,
        profile: 'local',
        created: true,
        default_profile: 'local',
        token_saved: false,
        token_available: true,
        values: {
          base_url: 'http://localhost:8080/admin',
          access_token: '',
          mcp_control_url: 'http://localhost:8931/control-session',
          mcp_client_id: 'bpane-mcp-bridge',
        },
      });
      const written = JSON.parse(await fs.readFile(filePath, 'utf8'));
      expect(written).toEqual({
        default_profile: 'local',
        profiles: {
          local: {
            base_url: 'http://localhost:8080/admin',
            mcp_control_url: 'http://localhost:8931/control-session',
            mcp_client_id: 'bpane-mcp-bridge',
          },
        },
      });
    } finally {
      await fs.rm(dir, { recursive: true, force: true });
    }
  });

  it('updates an existing CLI profile and saves tokens only when requested', async () => {
    await withConfig({
      default_profile: 'old',
      profiles: {
        local: {
          base_url: 'http://old.example',
          mcp_client_id: 'old-client',
        },
        old: {
          base_url: 'http://old-default.example',
        },
      },
    }, async (filePath) => {
      const io = createIo();
      const code = await runBpaneCli(
        [
          'profile',
          'init',
          'local',
          '--config',
          filePath,
          '--base-url',
          'http://new.example',
          '--access-token',
          'abcdefghijklmnop',
          '--save-token',
        ],
        {},
        io.io,
        async () => {
          throw new Error('profile init must not fetch');
        },
      );

      expect(code).toBe(EXIT_CODES.ok);
      expect(parseStdout(io)).toMatchObject({
        created: false,
        default_profile: 'old',
        token_saved: true,
        values: {
          base_url: 'http://new.example',
          access_token: 'abcd...mnop',
          mcp_client_id: 'old-client',
        },
      });
      const written = JSON.parse(await fs.readFile(filePath, 'utf8'));
      expect(written.profiles.local).toMatchObject({
        base_url: 'http://new.example',
        access_token: 'abcdefghijklmnop',
        mcp_client_id: 'old-client',
      });
      expect(written.default_profile).toBe('old');
      const stat = await fs.stat(filePath);
      expect(stat.mode & 0o777).toBe(0o600);
    });
  });

  it('lists and shows local CLI profiles with redacted tokens', async () => {
    await withConfig({
      default_profile: 'local',
      profiles: {
        local: {
          base_url: 'http://localhost:8080',
          access_token: 'abcdefghijklmnop',
          mcp_control_url: 'http://localhost:8931/control-session',
          mcp_client_id: 'bpane-mcp-bridge',
        },
        remote: {
          baseUrl: 'https://bpane.example',
        },
      },
    }, async (filePath) => {
      const listIo = createIo();
      const listCode = await runBpaneCli(['profile', 'list', '--config', filePath], {}, listIo.io, async () => {
        throw new Error('profile list must not fetch');
      });

      expect(listCode).toBe(EXIT_CODES.ok);
      expect(parseStdout(listIo)).toMatchObject({
        config_path: filePath,
        config_exists: true,
        active_profile: 'local',
        profiles: ['local', 'remote'],
      });

      const showIo = createIo();
      const showCode = await runBpaneCli(['profile', 'show', 'local', '--config', filePath], {}, showIo.io, async () => {
        throw new Error('profile show must not fetch');
      });

      expect(showCode).toBe(EXIT_CODES.ok);
      expect(parseStdout(showIo)).toMatchObject({
        profile: 'local',
        values: {
          base_url: 'http://localhost:8080',
          access_token: 'abcd...mnop',
          mcp_control_url: 'http://localhost:8931/control-session',
          mcp_client_id: 'bpane-mcp-bridge',
        },
      });
    });
  });

  it('loads gateway and MCP settings from the selected profile', async () => {
    await withConfig({
      profiles: {
        local: {
          baseUrl: 'http://profile.example',
          accessToken: 'profile-token',
        },
      },
    }, async (filePath) => {
      const io = createIo();
      const { calls, fetchImpl } = createFetch(jsonResponse({ sessions: [] }));

      const code = await runBpaneCli(
        ['session', 'list', '--config', filePath, '--profile', 'local'],
        {},
        io.io,
        fetchImpl,
      );

      expect(code).toBe(EXIT_CODES.ok);
      expect(calls[0].url).toBe('http://profile.example/api/v1/sessions');
      expect(calls[0].init.headers.Authorization).toBe('Bearer profile-token');
    });
  });

  it('lets flags and environment variables override profile values', async () => {
    await withConfig({
      profiles: {
        local: {
          baseUrl: 'http://profile.example',
          accessToken: 'profile-token',
        },
      },
    }, async (filePath) => {
      const io = createIo();
      const { calls, fetchImpl } = createFetch(jsonResponse({ sessions: [] }));

      const code = await runBpaneCli(
        ['session', 'list', '--config', filePath, '--profile', 'local', '--base-url', 'http://flag.example'],
        { BPANE_ACCESS_TOKEN: 'env-token' },
        io.io,
        fetchImpl,
      );

      expect(code).toBe(EXIT_CODES.ok);
      expect(calls[0].url).toBe('http://flag.example/api/v1/sessions');
      expect(calls[0].init.headers.Authorization).toBe('Bearer env-token');
    });
  });

  it('reports the authenticated identity through the gateway API', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({
      subject: 'legacy-dev-token:abc123',
      issuer: 'bpane-gateway',
      display_name: null,
      client_id: null,
      principal_type: 'legacy_dev_token',
    }));

    const code = await runBpaneCli(
      ['identity', 'me', '--base-url', 'http://bpane.example/root/'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      subject: 'legacy-dev-token:abc123',
      principal_type: 'legacy_dev_token',
    });
    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe('http://bpane.example/api/v1/identity/me');
    expect(calls[0].init.headers.Authorization).toBe('Bearer token-1');
  });

  it('reports the access-review payload through the gateway API', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({
      principal: {
        subject: 'demo',
        issuer: 'http://localhost:8091/realms/browserpane',
        display_name: 'demo',
        client_id: 'bpane-web',
        principal_type: 'user',
      },
      generated_at: '2026-05-29T10:00:00Z',
      projects: [{ id: 'project-1', name: 'support', usage: { active_sessions: 1 } }],
      resource_counts: { projects: 1, sessions: 1, delegated_principals: 1 },
      delegated_principals: [{ client_id: 'bpane-mcp-bridge', session_count: 1 }],
    }));

    const code = await runBpaneCli(
      ['identity', 'access-review'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      principal: { subject: 'demo', principal_type: 'user' },
      resource_counts: { projects: 1, sessions: 1, delegated_principals: 1 },
    });
    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/identity/access-review');
    expect(calls[0].init.headers.Authorization).toBe('Bearer token-1');
  });

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

  it('sends session list filters to the catalog API', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({
      sessions: [
        {
          id: 'session-1',
          state: 'stopped',
          labels: { suite: 'cli' },
          created_at: '2026-05-18T10:00:00Z',
        },
        {
          id: 'session-2',
          state: 'stopped',
          labels: { suite: 'cli' },
          created_at: '2026-05-18T10:00:00Z',
        },
        {
          id: 'session-3',
          state: 'running',
          labels: { suite: 'cli' },
          created_at: '2026-05-18T10:00:00Z',
        },
        {
          id: 'session-4',
          state: 'stopped',
          labels: { suite: 'other' },
          created_at: '2026-05-18T10:00:00Z',
        },
      ],
    }));

    const code = await runBpaneCli(
      [
        'session',
        'list',
        '--state',
        'stopped',
        '--runtime-state',
        'running',
        '--label',
        'suite=cli',
        '--integration',
        'ticket=INC-1',
        '--template-id',
        'template-1',
        '--limit',
        '1',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io).sessions).toHaveLength(4);
    expect(calls).toHaveLength(1);
    const url = new URL(calls[0].url);
    expect(`${url.origin}${url.pathname}`).toBe('http://localhost:8080/api/v1/sessions');
    expect(url.searchParams.get('state')).toBe('stopped');
    expect(url.searchParams.get('runtime_state')).toBe('running');
    expect(url.searchParams.get('label.suite')).toBe('cli');
    expect(url.searchParams.get('integration.ticket')).toBe('INC-1');
    expect(url.searchParams.get('template_id')).toBe('template-1');
    expect(url.searchParams.get('limit')).toBe('1');
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

  it('fetches session egress diagnostics by id', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ health: 'ready' }));

    const code = await runBpaneCli(
      ['session', 'egress-diagnostics', 'session/with space'],
      { BPANE_BASE_URL: 'http://localhost:8080', BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ health: 'ready' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/sessions/session%2Fwith%20space/egress-diagnostics');
  });

  it('runs a session egress diagnostics probe by id', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ proof_level: 'active_probe' }));

    const code = await runBpaneCli(
      [
        'session',
        'egress-diagnostics',
        'probe',
        'session/with space',
        '--probe-public-ip-url',
        'https://probe.example/ip',
        '--probe-tls-url',
        'https://probe.example/tls',
        '--probe-timeout-ms',
        '1000',
      ],
      { BPANE_BASE_URL: 'http://localhost:8080', BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ proof_level: 'active_probe' });
    expect(calls[0].init.method).toBe('POST');
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/sessions/session%2Fwith%20space/egress-diagnostics');
    expect(JSON.parse(String(calls[0].init.body))).toEqual({
      public_ip_url: 'https://probe.example/ip',
      tls_probe_url: 'https://probe.example/tls',
      timeout_ms: 1000,
    });
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
        '--project-id',
        '0198dff7-457e-7000-8000-000000000025',
        '--template-id',
        'desktop',
        '--browser-context-id',
        '0198dff7-457e-7000-8000-000000000015',
        '--owner-mode',
        'collaborative',
        '--width',
        '1440',
        '--height',
        '900',
        '--idle-timeout-sec',
        '120',
        '--locale',
        'de-DE',
        '--language',
        'de-DE',
        '--language',
        'en-US',
        '--timezone',
        'Europe/Berlin',
        '--geolocation-latitude',
        '52.52',
        '--geolocation-longitude',
        '13.405',
        '--geolocation-accuracy-meters',
        '100',
        '--browser-identity',
        'desktop-chromium-stable',
        '--egress-profile-id',
        '0198dff7-457e-7000-8000-000000000099',
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
      project_id: '0198dff7-457e-7000-8000-000000000025',
      template_id: 'desktop',
      browser_context: {
        mode: 'reusable',
        context_id: '0198dff7-457e-7000-8000-000000000015',
      },
      owner_mode: 'collaborative',
      viewport: { width: 1440, height: 900 },
      idle_timeout_sec: 120,
      network_identity: {
        locale: 'de-DE',
        languages: ['de-DE', 'en-US'],
        timezone: 'Europe/Berlin',
        geolocation: {
          latitude: 52.52,
          longitude: 13.405,
          accuracy_meters: 100,
        },
        browser_identity: 'desktop-chromium-stable',
        egress_profile_id: '0198dff7-457e-7000-8000-000000000099',
      },
      labels: { suite: 'cli' },
      integration_context: { ticket: 'abc' },
      extension_ids: ['018f1a5b-0784-71bf-ae46-0c973f00aa11'],
      recording: { mode: 'manual', format: 'webm', retention_sec: 3600 },
    });
  });

  it('manages browser contexts through the CLI', async () => {
    const createContextIo = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ id: 'context-1' }, 201),
      jsonResponse({ id: 'context-2', name: 'support-profile-copy' }, 201),
      jsonResponse({ contexts: [{ id: 'context-1' }] }),
      jsonResponse({ id: 'context-1' }),
      binaryResponse('PKbrowser-context-export', 'application/zip'),
      jsonResponse({ id: 'context-3', name: 'support-profile-import' }, 201),
      jsonResponse({ id: 'context-1', state: 'deleted' }),
    );

    const createCode = await runBpaneCli(
      [
        'browser-context',
        'create',
        'support-profile',
        '--description',
        'Support profile',
        '--label',
        'team=support',
        '--retention-sec',
        '604800',
        '--max-profile-storage-bytes',
        '67108864',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      createContextIo.io,
      fetchImpl,
    );
    expect(createCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(createContextIo)).toEqual({ id: 'context-1' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/browser-contexts');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      name: 'support-profile',
      description: 'Support profile',
      labels: { team: 'support' },
      retention_sec: 604800,
      max_profile_storage_bytes: 67108864,
    });

    const cloneIo = createIo();
    const cloneCode = await runBpaneCli(
      [
        'browser-context',
        'clone',
        'context-1',
        'support-profile-copy',
        '--label',
        'copy=sandbox',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      cloneIo.io,
      fetchImpl,
    );
    expect(cloneCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(cloneIo)).toEqual({ id: 'context-2', name: 'support-profile-copy' });
    expect(calls[1].url).toBe('http://localhost:8080/api/v1/browser-contexts/context-1/clone');
    expect(calls[1].init.method).toBe('POST');
    expect(JSON.parse(calls[1].init.body)).toEqual({
      name: 'support-profile-copy',
      labels: { copy: 'sandbox' },
    });

    const listIo = createIo();
    const listCode = await runBpaneCli(
      ['browser-context', 'list'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      listIo.io,
      fetchImpl,
    );
    expect(listCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(listIo)).toEqual({ contexts: [{ id: 'context-1' }] });

    const getIo = createIo();
    const getCode = await runBpaneCli(
      ['browser-context', 'get', 'context/with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      getIo.io,
      fetchImpl,
    );
    expect(getCode).toBe(EXIT_CODES.ok);

    const exportDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-cli-export-test-'));
    const exportPath = path.join(exportDir, 'context.zip');
    const exportIo = createIo();
    const exportCode = await runBpaneCli(
      ['browser-context', 'export', 'context-1', '--output', exportPath],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      exportIo.io,
      fetchImpl,
    );
    expect(exportCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(exportIo)).toMatchObject({
      context_id: 'context-1',
      output_path: exportPath,
      byte_count: 24,
      content_type: 'application/zip',
    });
    expect(await fs.readFile(exportPath, 'utf8')).toBe('PKbrowser-context-export');

    const importIo = createIo();
    const importCode = await runBpaneCli(
      [
        'browser-context',
        'import',
        '--input',
        exportPath,
        '--name',
        'support-profile-import',
        '--label',
        'imported=true',
        '--retention-sec',
        '43200',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      importIo.io,
      fetchImpl,
    );
    expect(importCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(importIo)).toEqual({ id: 'context-3', name: 'support-profile-import' });
    await fs.rm(exportDir, { recursive: true, force: true });

    const deleteIo = createIo();
    const deleteCode = await runBpaneCli(
      ['browser-context', 'delete', 'context-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      deleteIo.io,
      fetchImpl,
    );
    expect(deleteCode).toBe(EXIT_CODES.ok);

    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/browser-contexts', 'POST'],
      ['http://localhost:8080/api/v1/browser-contexts/context-1/clone', 'POST'],
      ['http://localhost:8080/api/v1/browser-contexts', undefined],
      ['http://localhost:8080/api/v1/browser-contexts/context%2Fwith%20space', undefined],
      ['http://localhost:8080/api/v1/browser-contexts/context-1/export', 'GET'],
      ['http://localhost:8080/api/v1/browser-contexts/import', 'POST'],
      ['http://localhost:8080/api/v1/browser-contexts/context-1', 'DELETE'],
    ]);
    expect(calls[5].init.headers).toMatchObject({
      'Content-Type': 'application/zip',
      'x-bpane-browser-context-name': 'support-profile-import',
      'x-bpane-browser-context-labels': JSON.stringify({ imported: 'true' }),
      'x-bpane-browser-context-retention-sec': '43200',
    });
  });

  it('validates browser context CLI input before fetching', async () => {
    const missingNameIo = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ ok: true }));

    const missingNameCode = await runBpaneCli(
      ['browser-context', 'create'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingNameIo.io,
      fetchImpl,
    );
    expect(missingNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingNameIo).code).toBe('USAGE');

    const invalidRetentionIo = createIo();
    const invalidRetentionCode = await runBpaneCli(
      ['browser-context', 'create', 'support-profile', '--retention-sec', '0'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      invalidRetentionIo.io,
      fetchImpl,
    );
    expect(invalidRetentionCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(invalidRetentionIo).error).toContain('--retention-sec');

    const invalidStorageLimitIo = createIo();
    const invalidStorageLimitCode = await runBpaneCli(
      ['browser-context', 'create', 'support-profile', '--max-profile-storage-bytes', '0'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      invalidStorageLimitIo.io,
      fetchImpl,
    );
    expect(invalidStorageLimitCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(invalidStorageLimitIo).error).toContain('--max-profile-storage-bytes');

    const missingExportOutputIo = createIo();
    const missingExportOutputCode = await runBpaneCli(
      ['browser-context', 'export', 'context-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingExportOutputIo.io,
      fetchImpl,
    );
    expect(missingExportOutputCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingExportOutputIo).error).toContain('--output');

    const missingImportInputIo = createIo();
    const missingImportInputCode = await runBpaneCli(
      ['browser-context', 'import', '--name', 'support-profile-import'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingImportInputIo.io,
      fetchImpl,
    );
    expect(missingImportInputCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingImportInputIo).error).toContain('--input');

    const missingImportNameIo = createIo();
    const missingImportNameCode = await runBpaneCli(
      ['browser-context', 'import', '--input', '/tmp/context.zip'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingImportNameIo.io,
      fetchImpl,
    );
    expect(missingImportNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingImportNameIo).error).toContain('--name');

    const missingCloneNameIo = createIo();
    const missingCloneNameCode = await runBpaneCli(
      ['browser-context', 'clone', 'context-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingCloneNameIo.io,
      fetchImpl,
    );
    expect(missingCloneNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingCloneNameIo).error).toContain('Browser context clone requires');

    const missingContextIo = createIo();
    const missingContextCode = await runBpaneCli(
      ['session', 'create', '--browser-context-mode', 'reusable'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingContextIo.io,
      fetchImpl,
    );
    expect(missingContextCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingContextIo).error).toContain('--browser-context-id is required');

    const incompatibleContextIo = createIo();
    const incompatibleContextCode = await runBpaneCli(
      [
        'session',
        'create',
        '--browser-context-mode',
        'fresh',
        '--browser-context-id',
        'context-1',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      incompatibleContextIo.io,
      fetchImpl,
    );
    expect(incompatibleContextCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(incompatibleContextIo).error).toContain(
      '--browser-context-id can only be used',
    );

    const invalidGeolocationIo = createIo();
    const invalidGeolocationCode = await runBpaneCli(
      [
        'session',
        'create',
        '--geolocation-latitude',
        '52.52',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      invalidGeolocationIo.io,
      fetchImpl,
    );
    expect(invalidGeolocationCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(invalidGeolocationIo).error).toContain(
      'Use --geolocation-latitude and --geolocation-longitude together',
    );
    expect(calls).toHaveLength(0);
  });

  it('creates session templates with structured defaults', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ id: 'template-1' }, 201));

    const code = await runBpaneCli(
      [
        'session-template',
        'create',
        'customer-debug-session',
      '--description',
      'Support debug session',
      '--project-id',
      'project-1',
      '--label',
        'team=support',
        '--default-label',
        'purpose=debug',
        '--owner-mode',
        'collaborative',
        '--width',
        '1440',
        '--height',
        '900',
        '--idle-timeout-sec',
        '1800',
        '--locale',
        'de-DE',
        '--language',
        'de-DE,en-US',
        '--timezone',
        'Europe/Berlin',
        '--egress-profile-id',
        'egress-1',
        '--integration-json',
        '{"source":"template"}',
        '--recording-mode',
        'manual',
        '--recording-retention-sec',
        '86400',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({ id: 'template-1' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/session-templates');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      name: 'customer-debug-session',
      description: 'Support debug session',
      labels: { team: 'support' },
      defaults: {
        project_id: 'project-1',
        owner_mode: 'collaborative',
        viewport: { width: 1440, height: 900 },
        idle_timeout_sec: 1800,
        network_identity: {
          locale: 'de-DE',
          languages: ['de-DE', 'en-US'],
          timezone: 'Europe/Berlin',
          egress_profile_id: 'egress-1',
        },
        labels: { purpose: 'debug' },
        integration_context: { source: 'template' },
        recording: { mode: 'manual', format: 'webm', retention_sec: 86400 },
      },
    });
  });

  it('manages projects through the CLI', async () => {
    const createProjectIo = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ id: 'project-1', name: 'support-tenant' }, 201),
      jsonResponse({ projects: [{ id: 'project-1' }] }),
      jsonResponse({ id: 'project-1', name: 'support-tenant' }),
      jsonResponse({
        id: 'project-1',
        name: 'support-tenant',
        description: 'Existing project',
        labels: { tenant: 'support' },
        quotas: {
          max_active_sessions: 2,
          max_active_workflow_runs: 4,
          max_retained_storage_bytes: 1073741824,
        },
        state: 'active',
      }),
      jsonResponse({ id: 'project-1', name: 'support-tenant-v2', state: 'active' }),
      jsonResponse({
        project_id: 'project-1',
        active_sessions: 1,
        max_active_sessions: 3,
        active_workflow_runs: 0,
        max_active_workflow_runs: 4,
        retained_storage_bytes: 0,
        max_retained_storage_bytes: 1073741824,
      }),
      jsonResponse({
        id: 'project-1',
        name: 'support-tenant-v2',
        labels: { tenant: 'support', managed: 'true' },
        quotas: { max_active_sessions: 3 },
        state: 'active',
      }),
      jsonResponse({ id: 'project-1', name: 'support-tenant-v2', state: 'archived' }),
    );

    const createCode = await runBpaneCli(
      [
        'project',
        'create',
        'support-tenant',
        '--description',
        'Support tenant',
        '--label',
        'tenant=support',
        '--max-active-sessions',
        '2',
        '--max-active-workflow-runs',
        '4',
        '--max-retained-storage-bytes',
        '1073741824',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      createProjectIo.io,
      fetchImpl,
    );
    expect(createCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(createProjectIo)).toEqual({ id: 'project-1', name: 'support-tenant' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/projects');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      name: 'support-tenant',
      description: 'Support tenant',
      labels: { tenant: 'support' },
      quotas: {
        max_active_sessions: 2,
        max_active_workflow_runs: 4,
        max_retained_storage_bytes: 1073741824,
      },
    });

    const listIo = createIo();
    const listCode = await runBpaneCli(
      ['project', 'list'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      listIo.io,
      fetchImpl,
    );
    expect(listCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(listIo)).toEqual({ projects: [{ id: 'project-1' }] });

    const getIo = createIo();
    const getCode = await runBpaneCli(
      ['project', 'get', 'project/with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      getIo.io,
      fetchImpl,
    );
    expect(getCode).toBe(EXIT_CODES.ok);
    expect(calls[2].url).toBe('http://localhost:8080/api/v1/projects/project%2Fwith%20space');

    const updateIo = createIo();
    const updateCode = await runBpaneCli(
      [
        'project',
        'update',
        'project-1',
        '--name',
        'support-tenant-v2',
        '--label',
        'managed=true',
        '--max-active-sessions',
        '3',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      updateIo.io,
      fetchImpl,
    );
    expect(updateCode).toBe(EXIT_CODES.ok);
    expect(calls[3].url).toBe('http://localhost:8080/api/v1/projects/project-1');
    expect(calls[4].url).toBe('http://localhost:8080/api/v1/projects/project-1');
    expect(calls[4].init.method).toBe('PUT');
    expect(JSON.parse(calls[4].init.body)).toEqual({
      name: 'support-tenant-v2',
      description: 'Existing project',
      labels: { tenant: 'support', managed: 'true' },
      quotas: {
        max_active_sessions: 3,
        max_active_workflow_runs: 4,
        max_retained_storage_bytes: 1073741824,
      },
      state: 'active',
    });

    const usageIo = createIo();
    const usageCode = await runBpaneCli(
      ['project', 'usage', 'project-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      usageIo.io,
      fetchImpl,
    );
    expect(usageCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(usageIo).active_sessions).toBe(1);
    expect(calls[5].url).toBe('http://localhost:8080/api/v1/projects/project-1/usage');

    const archiveIo = createIo();
    const archiveCode = await runBpaneCli(
      ['project', 'archive', 'project-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      archiveIo.io,
      fetchImpl,
    );
    expect(archiveCode).toBe(EXIT_CODES.ok);
    expect(calls[7].init.method).toBe('PUT');
    expect(JSON.parse(calls[7].init.body)).toEqual({
      name: 'support-tenant-v2',
      labels: { tenant: 'support', managed: 'true' },
      quotas: { max_active_sessions: 3 },
      state: 'archived',
    });
  });

  it('manages service principals through the CLI', async () => {
    const createServicePrincipalIo = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ id: 'service-principal-1', name: 'MCP bridge' }, 201),
      jsonResponse({ service_principals: [{ id: 'service-principal-1' }] }),
      jsonResponse({ id: 'service-principal-1', name: 'MCP bridge' }),
      jsonResponse({
        id: 'service-principal-1',
        name: 'MCP bridge',
        description: 'Existing service principal',
        client_id: 'bpane-mcp-bridge',
        issuer: 'https://issuer.example',
        labels: { system: 'mcp' },
        scopes: ['session:delegate'],
        allowed_project_ids: ['project-1'],
        state: 'active',
      }),
      jsonResponse({ id: 'service-principal-1', name: 'MCP bridge v2', state: 'active' }),
      jsonResponse({
        id: 'service-principal-1',
        name: 'MCP bridge v2',
        client_id: 'bpane-mcp-bridge',
        issuer: 'https://issuer.example',
        labels: { system: 'mcp', managed: 'true' },
        scopes: ['session:delegate', 'workflow:run'],
        allowed_project_ids: ['project-1'],
        state: 'active',
      }),
      jsonResponse({ id: 'service-principal-1', name: 'MCP bridge v2', state: 'disabled' }),
    );

    const createCode = await runBpaneCli(
      [
        'service-principal',
        'create',
        'MCP bridge',
        '--description',
        'Bridge automation identity',
        '--client-id',
        'bpane-mcp-bridge',
        '--issuer',
        'https://issuer.example',
        '--label',
        'system=mcp',
        '--scope',
        'session:delegate',
        '--allowed-project-id',
        'project-1',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      createServicePrincipalIo.io,
      fetchImpl,
    );
    expect(createCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(createServicePrincipalIo)).toEqual({ id: 'service-principal-1', name: 'MCP bridge' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/service-principals');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      name: 'MCP bridge',
      description: 'Bridge automation identity',
      client_id: 'bpane-mcp-bridge',
      issuer: 'https://issuer.example',
      labels: { system: 'mcp' },
      scopes: ['session:delegate'],
      allowed_project_ids: ['project-1'],
    });

    const listIo = createIo();
    const listCode = await runBpaneCli(
      ['service-principal', 'list'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      listIo.io,
      fetchImpl,
    );
    expect(listCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(listIo)).toEqual({ service_principals: [{ id: 'service-principal-1' }] });

    const getIo = createIo();
    const getCode = await runBpaneCli(
      ['service-principal', 'get', 'service/principal with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      getIo.io,
      fetchImpl,
    );
    expect(getCode).toBe(EXIT_CODES.ok);
    expect(calls[2].url).toBe('http://localhost:8080/api/v1/service-principals/service%2Fprincipal%20with%20space');

    const updateIo = createIo();
    const updateCode = await runBpaneCli(
      [
        'service-principal',
        'update',
        'service-principal-1',
        '--name',
        'MCP bridge v2',
        '--label',
        'managed=true',
        '--scope',
        'workflow:run',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      updateIo.io,
      fetchImpl,
    );
    expect(updateCode).toBe(EXIT_CODES.ok);
    expect(calls[3].url).toBe('http://localhost:8080/api/v1/service-principals/service-principal-1');
    expect(calls[4].init.method).toBe('PUT');
    expect(JSON.parse(calls[4].init.body)).toEqual({
      name: 'MCP bridge v2',
      description: 'Existing service principal',
      client_id: 'bpane-mcp-bridge',
      issuer: 'https://issuer.example',
      labels: { system: 'mcp', managed: 'true' },
      scopes: ['workflow:run'],
      allowed_project_ids: ['project-1'],
      state: 'active',
    });

    const disableIo = createIo();
    const disableCode = await runBpaneCli(
      ['service-principal', 'disable', 'service-principal-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      disableIo.io,
      fetchImpl,
    );
    expect(disableCode).toBe(EXIT_CODES.ok);
    expect(calls[6].init.method).toBe('PUT');
    expect(JSON.parse(calls[6].init.body)).toEqual({
      name: 'MCP bridge v2',
      client_id: 'bpane-mcp-bridge',
      issuer: 'https://issuer.example',
      labels: { system: 'mcp', managed: 'true' },
      scopes: ['session:delegate', 'workflow:run'],
      allowed_project_ids: ['project-1'],
      state: 'disabled',
    });
  });

  it('validates project CLI input before fetching', async () => {
    const missingNameIo = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ ok: true }));

    const missingNameCode = await runBpaneCli(
      ['project', 'create'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingNameIo.io,
      fetchImpl,
    );
    expect(missingNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingNameIo).error).toContain('Project create requires');

    const invalidQuotaIo = createIo();
    const invalidQuotaCode = await runBpaneCli(
      ['project', 'create', 'support-tenant', '--max-active-sessions', '0'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      invalidQuotaIo.io,
      fetchImpl,
    );
    expect(invalidQuotaCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(invalidQuotaIo).error).toContain('--max-active-sessions');
    expect(calls).toHaveLength(0);
  });

  it('validates service-principal CLI input before fetching', async () => {
    const missingNameIo = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ ok: true }));

    const missingNameCode = await runBpaneCli(
      ['service-principal', 'create', '--client-id', 'bpane-mcp-bridge', '--issuer', 'https://issuer.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingNameIo.io,
      fetchImpl,
    );
    expect(missingNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingNameIo).error).toContain('Service principal create requires');

    const missingClientIo = createIo();
    const missingClientCode = await runBpaneCli(
      ['service-principal', 'create', 'MCP bridge', '--issuer', 'https://issuer.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingClientIo.io,
      fetchImpl,
    );
    expect(missingClientCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingClientIo).error).toContain('requires --client-id');

    const missingIssuerIo = createIo();
    const missingIssuerCode = await runBpaneCli(
      ['service-principal', 'create', 'MCP bridge', '--client-id', 'bpane-mcp-bridge'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingIssuerIo.io,
      fetchImpl,
    );
    expect(missingIssuerCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingIssuerIo).error).toContain('requires --issuer');
    expect(calls).toHaveLength(0);
  });

  it('manages identity mappings through the CLI', async () => {
    const createMappingIo = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ id: 'mapping-1', name: 'MCP bridge mapping' }, 201),
      jsonResponse({ identity_mappings: [{ id: 'mapping-1' }] }),
      jsonResponse({ id: 'mapping-1', name: 'MCP bridge mapping' }),
      jsonResponse({
        id: 'mapping-1',
        name: 'MCP bridge mapping',
        description: 'Existing identity mapping',
        kind: 'service_principal',
        issuer: 'https://issuer.example',
        external_id: 'bpane-mcp-bridge',
        service_principal_id: 'service-principal-1',
        project_id: 'project-1',
        labels: { system: 'mcp' },
        scopes: ['session:create'],
        state: 'active',
      }),
      jsonResponse({ id: 'mapping-1', name: 'MCP bridge mapping v2', state: 'active' }),
      jsonResponse({
        id: 'mapping-1',
        name: 'MCP bridge mapping v2',
        kind: 'service_principal',
        issuer: 'https://issuer.example',
        external_id: 'bpane-mcp-bridge',
        service_principal_id: 'service-principal-1',
        project_id: 'project-1',
        labels: { system: 'mcp', managed: 'true' },
        scopes: ['session:create', 'session:delegate'],
        state: 'active',
      }),
      jsonResponse({ id: 'mapping-1', name: 'MCP bridge mapping v2', state: 'disabled' }),
    );

    const createCode = await runBpaneCli(
      [
        'identity-mapping',
        'create',
        'MCP bridge mapping',
        '--kind',
        'service_principal',
        '--issuer',
        'https://issuer.example',
        '--external-id',
        'bpane-mcp-bridge',
        '--service-principal-id',
        'service-principal-1',
        '--project-id',
        'project-1',
        '--label',
        'system=mcp',
        '--scope',
        'session:create',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      createMappingIo.io,
      fetchImpl,
    );
    expect(createCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(createMappingIo)).toEqual({ id: 'mapping-1', name: 'MCP bridge mapping' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/identity-mappings');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      name: 'MCP bridge mapping',
      kind: 'service_principal',
      issuer: 'https://issuer.example',
      external_id: 'bpane-mcp-bridge',
      service_principal_id: 'service-principal-1',
      project_id: 'project-1',
      labels: { system: 'mcp' },
      scopes: ['session:create'],
    });

    const listIo = createIo();
    const listCode = await runBpaneCli(
      ['identity-mapping', 'list'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      listIo.io,
      fetchImpl,
    );
    expect(listCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(listIo)).toEqual({ identity_mappings: [{ id: 'mapping-1' }] });

    const getIo = createIo();
    const getCode = await runBpaneCli(
      ['identity-mapping', 'get', 'mapping/with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      getIo.io,
      fetchImpl,
    );
    expect(getCode).toBe(EXIT_CODES.ok);
    expect(calls[2].url).toBe('http://localhost:8080/api/v1/identity-mappings/mapping%2Fwith%20space');

    const updateIo = createIo();
    const updateCode = await runBpaneCli(
      [
        'identity-mapping',
        'update',
        'mapping-1',
        '--name',
        'MCP bridge mapping v2',
        '--label',
        'managed=true',
        '--scope',
        'session:delegate',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      updateIo.io,
      fetchImpl,
    );
    expect(updateCode).toBe(EXIT_CODES.ok);
    expect(calls[3].url).toBe('http://localhost:8080/api/v1/identity-mappings/mapping-1');
    expect(calls[4].init.method).toBe('PUT');
    expect(JSON.parse(calls[4].init.body)).toEqual({
      name: 'MCP bridge mapping v2',
      description: 'Existing identity mapping',
      kind: 'service_principal',
      issuer: 'https://issuer.example',
      external_id: 'bpane-mcp-bridge',
      service_principal_id: 'service-principal-1',
      project_id: 'project-1',
      labels: { system: 'mcp', managed: 'true' },
      scopes: ['session:delegate'],
      state: 'active',
    });

    const disableIo = createIo();
    const disableCode = await runBpaneCli(
      ['identity-mapping', 'disable', 'mapping-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      disableIo.io,
      fetchImpl,
    );
    expect(disableCode).toBe(EXIT_CODES.ok);
    expect(calls[6].init.method).toBe('PUT');
    expect(JSON.parse(calls[6].init.body)).toEqual({
      name: 'MCP bridge mapping v2',
      kind: 'service_principal',
      issuer: 'https://issuer.example',
      external_id: 'bpane-mcp-bridge',
      service_principal_id: 'service-principal-1',
      project_id: 'project-1',
      labels: { system: 'mcp', managed: 'true' },
      scopes: ['session:create', 'session:delegate'],
      state: 'disabled',
    });
  });

  it('validates identity-mapping CLI input before fetching', async () => {
    const missingNameIo = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ ok: true }));

    const missingNameCode = await runBpaneCli(
      ['identity-mapping', 'create', '--kind', 'user', '--issuer', 'https://issuer.example', '--external-id', 'demo', '--project-id', 'project-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingNameIo.io,
      fetchImpl,
    );
    expect(missingNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingNameIo).error).toContain('Identity mapping create requires');

    const missingKindIo = createIo();
    const missingKindCode = await runBpaneCli(
      ['identity-mapping', 'create', 'Demo mapping', '--issuer', 'https://issuer.example', '--external-id', 'demo', '--project-id', 'project-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingKindIo.io,
      fetchImpl,
    );
    expect(missingKindCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingKindIo).error).toContain('requires --kind');

    const missingProjectIo = createIo();
    const missingProjectCode = await runBpaneCli(
      ['identity-mapping', 'create', 'Demo mapping', '--kind', 'user', '--issuer', 'https://issuer.example', '--external-id', 'demo'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingProjectIo.io,
      fetchImpl,
    );
    expect(missingProjectCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingProjectIo).error).toContain('requires --project-id');
    expect(calls).toHaveLength(0);
  });

  it('validates session-template CLI input before fetching', async () => {
    const missingNameIo = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ ok: true }));

    const missingNameCode = await runBpaneCli(
      ['session-template', 'create'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingNameIo.io,
      fetchImpl,
    );
    expect(missingNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingNameIo).code).toBe('USAGE');
    expect(calls).toHaveLength(0);

    const viewportIo = createIo();
    const viewportCode = await runBpaneCli(
      ['session-template', 'create', 'template', '--width', '1440'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      viewportIo.io,
      fetchImpl,
    );
    expect(viewportCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(viewportIo).error).toContain('Use --width and --height together');
    expect(calls).toHaveLength(0);

    const labelIo = createIo();
    const labelCode = await runBpaneCli(
      ['session-template', 'create', 'template', '--default-label', 'bad'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      labelIo.io,
      fetchImpl,
    );
    expect(labelCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(labelIo).error).toContain('--default-label must use key=value syntax');
    expect(calls).toHaveLength(0);
  });

  it('requires bearer tokens for session-template commands', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ templates: [] }));

    const code = await runBpaneCli(['session-template', 'list'], {}, io.io, fetchImpl);

    expect(code).toBe(EXIT_CODES.auth);
    expect(calls).toHaveLength(0);
    expect(parseStderr(io).code).toBe('AUTH_REQUIRED');
  });

  it('validates egress-profile CLI input before fetching', async () => {
    const missingNameIo = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ ok: true }));

    const missingNameCode = await runBpaneCli(
      ['egress-profile', 'create'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingNameIo.io,
      fetchImpl,
    );
    expect(missingNameCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingNameIo).error).toContain('Egress profile create requires');

    const missingCaRefIo = createIo();
    const missingCaRefCode = await runBpaneCli(
      ['egress-profile', 'create', 'eu-support-egress', '--custom-ca-name', 'EU support CA'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingCaRefIo.io,
      fetchImpl,
    );
    expect(missingCaRefCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingCaRefIo).error).toContain('--custom-ca-ref');

    const missingSinkIo = createIo();
    const missingSinkCode = await runBpaneCli(
      [
        'egress-profile',
        'create',
        'eu-support-egress',
        '--proxy-url',
        'https://proxy.example:8443',
        '--custom-ca-ref',
        'file:///workspace/dev/egress-ca.pem',
        '--traffic-observation-mode',
        'tls_intercept',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      missingSinkIo.io,
      fetchImpl,
    );
    expect(missingSinkCode).toBe(EXIT_CODES.usage);
    expect(parseStderr(missingSinkIo).error).toContain('--sensitive-log-sink-ref');
    expect(calls).toHaveLength(0);
  });

  it('lists, fetches, and updates session templates', async () => {
    const listIo = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ templates: [{ id: 'template-1' }] }),
      jsonResponse({ id: 'template-1' }),
      jsonResponse({
        id: 'template-1',
        name: 'customer-debug-session',
        description: 'Existing template',
        labels: { team: 'support' },
        defaults: {
          idle_timeout_sec: 1800,
          labels: { team: 'support' },
          network_identity: {
            locale: 'de-DE',
            timezone: 'Europe/Berlin',
            egress_profile_id: 'egress-1',
          },
          recording: { mode: 'manual', format: 'webm', retention_sec: 86400 },
        },
      }),
      jsonResponse({ id: 'template-1', version: 2 }),
    );

    const listCode = await runBpaneCli(
      ['session-template', 'list'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      listIo.io,
      fetchImpl,
    );
    expect(listCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(listIo)).toEqual({ templates: [{ id: 'template-1' }] });

    const getIo = createIo();
    const getCode = await runBpaneCli(
      ['session-template', 'get', 'template/with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      getIo.io,
      fetchImpl,
    );
    expect(getCode).toBe(EXIT_CODES.ok);

    const updateIo = createIo();
    const updateCode = await runBpaneCli(
      [
        'session-template',
        'update',
        'template-1',
        '--name',
        'customer-debug-session',
        '--default-label',
        'purpose=debug',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      updateIo.io,
      fetchImpl,
    );
    expect(updateCode).toBe(EXIT_CODES.ok);

    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/session-templates', undefined],
      ['http://localhost:8080/api/v1/session-templates/template%2Fwith%20space', undefined],
      ['http://localhost:8080/api/v1/session-templates/template-1', undefined],
      ['http://localhost:8080/api/v1/session-templates/template-1', 'PUT'],
    ]);
    expect(JSON.parse(calls[3].init.body)).toEqual({
      name: 'customer-debug-session',
      description: 'Existing template',
      labels: { team: 'support' },
      defaults: {
        idle_timeout_sec: 1800,
        labels: { team: 'support', purpose: 'debug' },
        network_identity: {
          locale: 'de-DE',
          timezone: 'Europe/Berlin',
          egress_profile_id: 'egress-1',
        },
        recording: { mode: 'manual', format: 'webm', retention_sec: 86400 },
      },
    });
  });

  it('manages egress profiles through the CLI', async () => {
    const createProfileIo = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ id: 'egress-1', name: 'eu-support-egress' }, 201),
      jsonResponse({ profiles: [{ id: 'egress-1' }] }),
      jsonResponse({ id: 'egress-1', name: 'eu-support-egress' }),
      jsonResponse({
        id: 'egress-1',
        name: 'eu-support-egress',
        description: 'EU support outbound path',
        labels: { region: 'eu' },
        proxy: { url: 'https://proxy.example:8443' },
        bypass_rules: ['localhost'],
        traffic_observation: { mode: 'metadata_only' },
        state: 'ready',
      }),
      jsonResponse({ id: 'egress-1', name: 'eu-support-egress-v2', state: 'ready' }),
      jsonResponse({
        id: 'egress-1',
        name: 'eu-support-egress-v2',
        proxy: { url: 'https://proxy.example:8443' },
        bypass_rules: ['localhost', '*.internal.example'],
        traffic_observation: { mode: 'metadata_only' },
        state: 'ready',
      }),
      jsonResponse({ id: 'egress-1', name: 'eu-support-egress-v2', state: 'disabled' }),
    );

    const createCode = await runBpaneCli(
      [
        'egress-profile',
        'create',
        'eu-support-egress',
        '--description',
        'EU support outbound path',
        '--label',
        'region=eu',
        '--proxy-url',
        'https://proxy.example:8443',
        '--proxy-credential-binding-id',
        'credential-binding-1',
        '--bypass-rule',
        'localhost',
        '--bypass-rule',
        '*.internal.example',
        '--custom-ca-ref',
        'file:///workspace/dev/egress-ca.pem',
        '--custom-ca-name',
        'EU support CA',
        '--traffic-observation-mode',
        'tls_intercept',
        '--sensitive-log-sink-ref',
        'siem://browserpane/eu-support',
        '--sensitive-log-sink-name',
        'EU support SIEM',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      createProfileIo.io,
      fetchImpl,
    );
    expect(createCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(createProfileIo)).toEqual({ id: 'egress-1', name: 'eu-support-egress' });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/egress-profiles');
    expect(calls[0].init.method).toBe('POST');
    expect(JSON.parse(calls[0].init.body)).toEqual({
      name: 'eu-support-egress',
      description: 'EU support outbound path',
      labels: { region: 'eu' },
      proxy: {
        url: 'https://proxy.example:8443',
        credential_binding_id: 'credential-binding-1',
      },
      bypass_rules: ['localhost', '*.internal.example'],
      custom_ca: {
        certificate_ref: 'file:///workspace/dev/egress-ca.pem',
        display_name: 'EU support CA',
      },
      traffic_observation: {
        mode: 'tls_intercept',
        sensitive_log_sink_ref: 'siem://browserpane/eu-support',
        sensitive_log_sink_display_name: 'EU support SIEM',
      },
    });

    const listIo = createIo();
    const listCode = await runBpaneCli(
      ['egress-profile', 'list'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      listIo.io,
      fetchImpl,
    );
    expect(listCode).toBe(EXIT_CODES.ok);
    expect(parseStdout(listIo)).toEqual({ profiles: [{ id: 'egress-1' }] });

    const getIo = createIo();
    const getCode = await runBpaneCli(
      ['egress-profile', 'get', 'egress/with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      getIo.io,
      fetchImpl,
    );
    expect(getCode).toBe(EXIT_CODES.ok);
    expect(calls[2].url).toBe('http://localhost:8080/api/v1/egress-profiles/egress%2Fwith%20space');

    const updateIo = createIo();
    const updateCode = await runBpaneCli(
      [
        'egress-profile',
        'update',
        'egress-1',
        '--name',
        'eu-support-egress-v2',
        '--label',
        'managed=true',
        '--bypass-rule',
        'localhost,*.internal.example',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      updateIo.io,
      fetchImpl,
    );
    expect(updateCode).toBe(EXIT_CODES.ok);
    expect(calls[3].url).toBe('http://localhost:8080/api/v1/egress-profiles/egress-1');
    expect(calls[4].url).toBe('http://localhost:8080/api/v1/egress-profiles/egress-1');
    expect(calls[4].init.method).toBe('PUT');
    expect(JSON.parse(calls[4].init.body)).toEqual({
      name: 'eu-support-egress-v2',
      description: 'EU support outbound path',
      labels: { region: 'eu', managed: 'true' },
      proxy: { url: 'https://proxy.example:8443' },
      bypass_rules: ['localhost', '*.internal.example'],
      traffic_observation: { mode: 'metadata_only' },
      state: 'ready',
    });

    const disableIo = createIo();
    const disableCode = await runBpaneCli(
      ['egress-profile', 'disable', 'egress-1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      disableIo.io,
      fetchImpl,
    );
    expect(disableCode).toBe(EXIT_CODES.ok);
    expect(calls[6].init.method).toBe('PUT');
    expect(JSON.parse(calls[6].init.body)).toEqual({
      name: 'eu-support-egress-v2',
      proxy: { url: 'https://proxy.example:8443' },
      bypass_rules: ['localhost', '*.internal.example'],
      traffic_observation: { mode: 'metadata_only' },
      state: 'disabled',
    });
  });

  it('fetches egress profile diagnostics through the CLI', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({
      profile_id: 'egress-1',
      health: 'ready',
      proof_level: 'configuration',
    }));

    const code = await runBpaneCli(
      ['egress-profile', 'diagnostics', 'egress/with space'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toEqual({
      profile_id: 'egress-1',
      health: 'ready',
      proof_level: 'configuration',
    });
    expect(calls[0].url).toBe('http://localhost:8080/api/v1/egress-profiles/egress%2Fwith%20space/diagnostics');
  });

  it('preserves equals signs in inline option values', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ id: 'session-1' }));

    const code = await runBpaneCli(
      [
        'session',
        'create',
        '--label=token=a=b',
        '--integration-json={"query":"a=b"}',
      ],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(JSON.parse(calls[0].init.body)).toEqual({
      labels: { token: 'a=b' },
      integration_context: { query: 'a=b' },
    });
  });

  it('rejects unsupported CLI options instead of ignoring typos', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(jsonResponse({ sessions: [] }));

    const code = await runBpaneCli(
      ['session', 'list', '--lable', 'suite=cli'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.usage);
    expect(calls).toHaveLength(0);
    expect(parseStderr(io)).toMatchObject({
      ok: false,
      code: 'USAGE',
      error: 'Unsupported option: --lable',
    });
  });

  it('mints access, automation access, cancels queue, and disconnects all session clients', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({ token_type: 'session_connect_ticket' }),
      jsonResponse({ token_type: 'session_automation_access_token' }),
      jsonResponse({ state: 'stopped' }),
      jsonResponse({ state: 'idle' }),
    );

    const env = { BPANE_ACCESS_TOKEN: 'token-1' };
    const accessCode = await runBpaneCli(['session', 'access-token', 'session-1'], env, io.io, fetchImpl);
    const automationCode = await runBpaneCli(['session', 'automation-access', 'session-1'], env, io.io, fetchImpl);
    const cancelCode = await runBpaneCli(['session', 'cancel', 'session-1'], env, io.io, fetchImpl);
    const disconnectCode = await runBpaneCli(['session', 'disconnect-all', 'session-1'], env, io.io, fetchImpl);

    expect(accessCode).toBe(EXIT_CODES.ok);
    expect(automationCode).toBe(EXIT_CODES.ok);
    expect(cancelCode).toBe(EXIT_CODES.ok);
    expect(disconnectCode).toBe(EXIT_CODES.ok);
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/sessions/session-1/access-tokens', 'POST'],
      ['http://localhost:8080/api/v1/sessions/session-1/automation-access', 'POST'],
      ['http://localhost:8080/api/v1/sessions/session-1/cancel', 'POST'],
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

  it('repairs MCP delegation and bridge default selection before strict diagnostics', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        mcpBridge: {
          controlUrl: 'http://mcp.example/control-session',
          clientId: 'bridge-client',
          issuer: 'issuer-1',
          displayName: 'Bridge Display',
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
      jsonResponse({ id: 'session-1', automation_delegate: { client_id: 'bridge-client' } }),
      jsonResponse({ session: { id: 'session-1' } }),
      jsonResponse({ status: 'ok', managed_sessions: [{ session_id: 'session-1' }] }),
      jsonResponse({ session: { id: 'session-1' } }),
      jsonResponse({
        id: 'session-1',
        state: 'running',
        automation_delegate: { client_id: 'bridge-client' },
      }),
      jsonResponse({ mcp_owner: false }),
    );

    const code = await runBpaneCli(
      ['mcp', 'repair', 'session-1', '--base-url', 'http://bpane.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      ok: true,
      session_id: 'session-1',
      failure_count: 0,
      actions: [
        { action: 'authorize', attempted: true, ok: true },
        { action: 'set-default', attempted: true, ok: true },
      ],
      diagnostics: { ok: true },
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://bpane.example/auth-config.json', undefined],
      ['http://mcp.example/health', undefined],
      ['http://mcp.example/control-session', 'GET'],
      ['http://bpane.example/api/v1/sessions/session-1', undefined],
      ['http://bpane.example/api/v1/sessions/session-1/status', undefined],
      ['http://bpane.example/api/v1/sessions/session-1/automation-owner', 'POST'],
      ['http://mcp.example/control-session', 'PUT'],
      ['http://mcp.example/health', undefined],
      ['http://mcp.example/control-session', 'GET'],
      ['http://bpane.example/api/v1/sessions/session-1', undefined],
      ['http://bpane.example/api/v1/sessions/session-1/status', undefined],
    ]);
    expect(JSON.parse(calls[5].init.body)).toEqual({
      client_id: 'bridge-client',
      issuer: 'issuer-1',
      display_name: 'Bridge Display',
    });
    expect(JSON.parse(calls[6].init.body)).toEqual({ session_id: 'session-1' });
  });

  it('returns non-zero when MCP repair cannot resolve final diagnostics', async () => {
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
        state: 'stopped',
        automation_delegate: null,
      }),
      jsonResponse({ mcp_owner: false }),
      jsonResponse({ id: 'session-1', automation_delegate: { client_id: 'bridge-client' } }),
      jsonResponse({ session: { id: 'session-1' } }),
      jsonResponse({ status: 'ok', managed_sessions: [{ session_id: 'session-1' }] }),
      jsonResponse({ session: { id: 'session-1' } }),
      jsonResponse({
        id: 'session-1',
        state: 'stopped',
        automation_delegate: { client_id: 'bridge-client' },
      }),
      jsonResponse({ mcp_owner: false }),
    );

    const code = await runBpaneCli(
      ['mcp', 'repair', 'session-1', '--base-url', 'http://bpane.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.api);
    expect(parseStdout(io)).toMatchObject({
      ok: false,
      failure_count: 0,
      diagnostics: {
        ok: false,
        issues: [{ code: 'SESSION_STOPPED' }],
      },
    });
  });

  it('refuses MCP repair mutations without an owner token', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        mcpBridge: {
          controlUrl: 'http://mcp.example/control-session',
          clientId: 'bridge-client',
        },
      }),
      jsonResponse({ status: 'ok', managed_sessions: [] }),
      jsonResponse({ session: null }),
    );

    const code = await runBpaneCli(
      ['mcp', 'repair', 'session-1', '--base-url', 'http://bpane.example'],
      {},
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.api);
    expect(parseStdout(io)).toMatchObject({
      ok: false,
      blocked: true,
      blocking_issues: [{ code: 'AUTH_REQUIRED' }],
      actions: [
        { action: 'authorize', attempted: false, ok: false, blocked: true },
        { action: 'set-default', attempted: false, ok: false, blocked: true },
      ],
      diagnostics: {
        ok: false,
        issues: [
          { code: 'AUTH_REQUIRED' },
          { code: 'MCP_DEFAULT_SESSION_MISMATCH' },
        ],
      },
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://bpane.example/auth-config.json', undefined],
      ['http://mcp.example/health', undefined],
      ['http://mcp.example/control-session', 'GET'],
    ]);
  });

  it('refuses MCP repair mutations when the session is not visible', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        mcpBridge: {
          controlUrl: 'http://mcp.example/control-session',
          clientId: 'bridge-client',
        },
      }),
      jsonResponse({ status: 'ok', managed_sessions: [] }),
      jsonResponse({ session: null }),
      jsonResponse({ error: 'session not found' }, 404),
      jsonResponse({ error: 'session not found' }, 404),
    );

    const code = await runBpaneCli(
      ['mcp', 'repair', 'missing-session', '--base-url', 'http://bpane.example'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.api);
    expect(parseStdout(io)).toMatchObject({
      ok: false,
      session_id: 'missing-session',
      blocked: true,
      blocking_issues: [{ code: 'SESSION_NOT_VISIBLE' }],
      actions: [
        { action: 'authorize', attempted: false, ok: false, blocked: true },
        { action: 'set-default', attempted: false, ok: false, blocked: true },
      ],
      diagnostics: {
        ok: false,
        issues: [
          { code: 'SESSION_NOT_VISIBLE' },
          { code: 'MCP_DEFAULT_SESSION_MISMATCH' },
        ],
      },
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://bpane.example/auth-config.json', undefined],
      ['http://mcp.example/health', undefined],
      ['http://mcp.example/control-session', 'GET'],
      ['http://bpane.example/api/v1/sessions/missing-session', undefined],
      ['http://bpane.example/api/v1/sessions/missing-session/status', undefined],
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
          id: 'session-3',
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
      ['session', 'cleanup', '--label', 'suite=cli', '--limit', '1'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      dry_run: true,
      planned_actions: ['revoke-automation-owner', 'disconnect-all', 'kill'],
      candidate_count: 1,
      matched_count: 2,
      total_count: 3,
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
      planned_actions: ['revoke-automation-owner', 'disconnect-all', 'kill'],
      result_count: 1,
      failure_count: 0,
      results: [{ session: { id: 'session-1' } }],
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/sessions', undefined],
      ['http://localhost:8080/api/v1/sessions/session-1/automation-owner', 'DELETE'],
      ['http://localhost:8080/api/v1/sessions/session-1/connections/disconnect-all', 'POST'],
      ['http://localhost:8080/api/v1/sessions/session-1/kill', 'POST'],
    ]);
  });

  it('executes selected cleanup actions only', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        sessions: [
          {
            id: 'session-1',
            state: 'stopped',
            labels: { suite: 'cli' },
            created_at: '2026-05-18T10:00:00Z',
          },
        ],
      }),
      jsonResponse({ id: 'session-1', state: 'stopped' }),
    );

    const code = await runBpaneCli(
      ['session', 'cleanup', '--label', 'suite=cli', '--cleanup-action', 'kill', '--confirm'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.ok);
    expect(parseStdout(io)).toMatchObject({
      dry_run: false,
      planned_actions: ['kill'],
      failure_count: 0,
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/sessions', undefined],
      ['http://localhost:8080/api/v1/sessions/session-1/kill', 'POST'],
    ]);
  });

  it('returns non-zero when confirmed cleanup operations fail', async () => {
    const io = createIo();
    const { calls, fetchImpl } = createFetch(
      jsonResponse({
        sessions: [
          {
            id: 'session-1',
            state: 'stopped',
            labels: { suite: 'cli' },
            created_at: '2026-05-18T10:00:00Z',
          },
        ],
      }),
      jsonResponse({ error: 'session has active blockers' }, 409),
    );

    const code = await runBpaneCli(
      ['session', 'cleanup', '--label', 'suite=cli', '--cleanup-action', 'stop', '--confirm'],
      { BPANE_ACCESS_TOKEN: 'token-1' },
      io.io,
      fetchImpl,
    );

    expect(code).toBe(EXIT_CODES.api);
    expect(parseStdout(io)).toMatchObject({
      dry_run: false,
      planned_actions: ['stop'],
      failure_count: 1,
      results: [
        {
          operations: [
            {
              operation: 'stop',
              ok: false,
              error: {
                code: 'HTTP_ERROR',
                status: 409,
                body: { error: 'session has active blockers' },
              },
            },
          ],
        },
      ],
    });
    expect(calls.map((call) => [call.url, call.init.method])).toEqual([
      ['http://localhost:8080/api/v1/sessions', undefined],
      ['http://localhost:8080/api/v1/sessions/session-1/stop', 'POST'],
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
