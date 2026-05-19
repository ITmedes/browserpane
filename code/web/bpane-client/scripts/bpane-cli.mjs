#!/usr/bin/env node

import process from 'node:process';
import { pathToFileURL } from 'node:url';

export const EXIT_CODES = Object.freeze({
  ok: 0,
  usage: 2,
  auth: 3,
  api: 4,
  unexpected: 5,
});

class CliError extends Error {
  constructor(code, message, exitCode, detail = {}) {
    super(message);
    this.name = 'CliError';
    this.code = code;
    this.exitCode = exitCode;
    this.detail = detail;
  }
}

function usageText() {
  return [
    'Usage:',
    '  bpane session list [options]',
    '  bpane session get <session-id> [options]',
    '  bpane session status <session-id> [options]',
    '  bpane session stop <session-id> [options]',
    '  bpane session kill <session-id> [options]',
    '  bpane mcp health [options]',
    '  bpane mcp authorize <session-id> [options]',
    '  bpane mcp revoke <session-id> [options]',
    '  bpane mcp set-default <session-id> [options]',
    '  bpane mcp clear-default [options]',
    '',
    'Options:',
    '  --base-url <url>          Gateway/web origin. Env: BPANE_BASE_URL or BPANE_API_URL. Default: http://localhost:8080.',
    '  --access-token <token>    Bearer token. Env: BPANE_ACCESS_TOKEN.',
    '  --token <token>           Alias for --access-token.',
    '  --mcp-control-url <url>   MCP bridge control URL. Env: BPANE_MCP_CONTROL_URL.',
    '  --mcp-client-id <id>      MCP delegate client id. Env: BPANE_MCP_CLIENT_ID.',
    '  --mcp-issuer <issuer>     MCP delegate issuer. Env: BPANE_MCP_ISSUER.',
    '  --mcp-display-name <name> MCP delegate display name. Env: BPANE_MCP_DISPLAY_NAME.',
    '  --help                    Show this help.',
    '',
    'All successful command responses are emitted as JSON.',
  ].join('\n');
}

function parseArgs(argv) {
  const positionals = [];
  const options = new Map();
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === '--') {
      positionals.push(...argv.slice(index + 1));
      break;
    }
    if (!token.startsWith('--')) {
      positionals.push(token);
      continue;
    }

    const [rawName, inlineValue] = token.split('=', 2);
    const name = rawName.slice(2);
    if (!name) {
      throw new CliError('USAGE', 'Encountered an empty option name.', EXIT_CODES.usage);
    }
    const next = argv[index + 1];
    const value =
      inlineValue !== undefined
        ? inlineValue
        : next !== undefined && !next.startsWith('--')
          ? (index += 1, next)
          : 'true';
    const existing = options.get(name);
    if (existing) {
      existing.push(value);
    } else {
      options.set(name, [value]);
    }
  }
  return { positionals, options };
}

function getOption(options, name, fallback = null) {
  const values = options.get(name);
  return values && values.length ? values[values.length - 1] : fallback;
}

function normalizeBaseUrl(value) {
  try {
    const url = new URL(value);
    url.pathname = url.pathname.replace(/\/+$/u, '');
    url.search = '';
    url.hash = '';
    return url.toString().replace(/\/$/u, '');
  } catch {
    throw new CliError('USAGE', `Invalid URL: ${value}`, EXIT_CODES.usage);
  }
}

function joinUrl(baseUrl, path) {
  return new URL(path, `${baseUrl}/`).toString();
}

function requiredSessionId(positionals, commandLabel) {
  const sessionId = positionals[2];
  if (!sessionId || positionals.length > 3) {
    throw new CliError('USAGE', `Usage: bpane ${commandLabel} <session-id>`, EXIT_CODES.usage);
  }
  return sessionId;
}

function requireAccessToken(config) {
  if (!config.accessToken) {
    throw new CliError(
      'AUTH_REQUIRED',
      'Missing bearer token. Pass --access-token/--token or set BPANE_ACCESS_TOKEN.',
      EXIT_CODES.auth,
    );
  }
  return config.accessToken;
}

function jsonHeaders(config, extraHeaders = {}) {
  const token = requireAccessToken(config);
  return {
    Authorization: `Bearer ${token}`,
    ...extraHeaders,
  };
}

function errorPayload(error) {
  if (error instanceof CliError) {
    return {
      ok: false,
      code: error.code,
      error: error.message,
      ...error.detail,
    };
  }
  return {
    ok: false,
    code: 'UNEXPECTED',
    error: error instanceof Error ? error.message : String(error),
  };
}

function printJson(io, value) {
  io.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

async function parseResponseBody(response) {
  const text = await response.text().catch(() => '');
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

async function requestJson(config, url, init = {}) {
  let response;
  try {
    response = await config.fetchImpl(url, init);
  } catch (error) {
    throw new CliError(
      'REQUEST_FAILED',
      error instanceof Error ? error.message : String(error),
      EXIT_CODES.api,
      { url },
    );
  }
  const body = await parseResponseBody(response);
  if (!response.ok) {
    throw new CliError(
      'HTTP_ERROR',
      `HTTP ${response.status}${typeof body === 'string' && body ? ` ${body}` : ''}`,
      EXIT_CODES.api,
      { status: response.status, body },
    );
  }
  return body;
}

async function requestGateway(config, path, init = {}) {
  const headers = jsonHeaders(config, init.headers);
  if (init.body !== undefined && !headers['Content-Type']) {
    headers['Content-Type'] = 'application/json';
  }
  return await requestJson(config, joinUrl(config.baseUrl, path), {
    ...init,
    headers,
  });
}

async function fetchAuthConfig(config) {
  try {
    return await requestJson(config, joinUrl(config.baseUrl, '/auth-config.json'));
  } catch {
    return null;
  }
}

function bridgeHealthUrl(controlUrl) {
  const url = new URL(controlUrl);
  if (/\/control-session\/?$/u.test(url.pathname)) {
    url.pathname = url.pathname.replace(/\/control-session\/?$/u, '/health');
  } else {
    url.pathname = '/health';
  }
  url.search = '';
  url.hash = '';
  return url.toString();
}

async function resolveMcpConfig(config, requirements = {}) {
  if (config.mcpConfig) {
    return config.mcpConfig;
  }
  const needsBridgeConfig =
    (requirements.control === true && !config.mcpControlUrl)
    || (requirements.client === true && !config.mcpClientId);
  const authConfig = needsBridgeConfig ? await fetchAuthConfig(config) : null;
  const bridge = authConfig?.mcpBridge ?? {};
  config.mcpConfig = {
    controlUrl:
      config.mcpControlUrl
      ?? bridge.controlUrl
      ?? 'http://localhost:8931/control-session',
    clientId:
      config.mcpClientId
      ?? bridge.clientId
      ?? 'bpane-mcp-bridge',
    issuer:
      config.mcpIssuer
      ?? bridge.issuer
      ?? null,
    displayName:
      config.mcpDisplayName
      ?? bridge.displayName
      ?? 'BrowserPane MCP bridge',
  };
  return config.mcpConfig;
}

function buildDelegateBody(mcpConfig) {
  const body = {
    client_id: mcpConfig.clientId,
  };
  if (mcpConfig.issuer) {
    body.issuer = mcpConfig.issuer;
  }
  if (mcpConfig.displayName) {
    body.display_name = mcpConfig.displayName;
  }
  return body;
}

async function handleSessionCommand(config, positionals) {
  const action = positionals[1];
  if (action === 'list' && positionals.length === 2) {
    return await requestGateway(config, '/api/v1/sessions');
  }
  if (action === 'get') {
    const sessionId = requiredSessionId(positionals, 'session get');
    return await requestGateway(config, `/api/v1/sessions/${encodeURIComponent(sessionId)}`);
  }
  if (action === 'status') {
    const sessionId = requiredSessionId(positionals, 'session status');
    return await requestGateway(
      config,
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/status`,
    );
  }
  if (action === 'stop') {
    const sessionId = requiredSessionId(positionals, 'session stop');
    return await requestGateway(config, `/api/v1/sessions/${encodeURIComponent(sessionId)}/stop`, {
      method: 'POST',
    });
  }
  if (action === 'kill') {
    const sessionId = requiredSessionId(positionals, 'session kill');
    return await requestGateway(config, `/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`, {
      method: 'POST',
    });
  }
  throw new CliError('USAGE', `Unknown session command: ${action ?? ''}`.trim(), EXIT_CODES.usage);
}

async function handleMcpCommand(config, positionals) {
  const action = positionals[1];
  if (action === 'health' && positionals.length === 2) {
    const mcpConfig = await resolveMcpConfig(config, { control: true });
    return await requestJson(config, bridgeHealthUrl(mcpConfig.controlUrl));
  }
  if (action === 'authorize') {
    const sessionId = requiredSessionId(positionals, 'mcp authorize');
    const mcpConfig = await resolveMcpConfig(config, { client: true });
    return await requestGateway(
      config,
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(buildDelegateBody(mcpConfig)),
      },
    );
  }
  if (action === 'revoke') {
    const sessionId = requiredSessionId(positionals, 'mcp revoke');
    return await requestGateway(
      config,
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
      { method: 'DELETE' },
    );
  }
  if (action === 'set-default') {
    const sessionId = requiredSessionId(positionals, 'mcp set-default');
    const mcpConfig = await resolveMcpConfig(config, { control: true });
    return await requestJson(config, mcpConfig.controlUrl, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ session_id: sessionId }),
    });
  }
  if (action === 'clear-default' && positionals.length === 2) {
    const mcpConfig = await resolveMcpConfig(config, { control: true });
    return await requestJson(config, mcpConfig.controlUrl, {
      method: 'DELETE',
    });
  }
  throw new CliError('USAGE', `Unknown MCP command: ${action ?? ''}`.trim(), EXIT_CODES.usage);
}

function buildConfig(options, env, fetchImpl) {
  return {
    baseUrl: normalizeBaseUrl(
      getOption(options, 'base-url')
      ?? getOption(options, 'api-url')
      ?? env.BPANE_BASE_URL
      ?? env.BPANE_API_URL
      ?? 'http://localhost:8080',
    ),
    accessToken:
      getOption(options, 'access-token')
      ?? getOption(options, 'token')
      ?? env.BPANE_ACCESS_TOKEN
      ?? '',
    mcpControlUrl:
      getOption(options, 'mcp-control-url')
      ?? env.BPANE_MCP_CONTROL_URL
      ?? null,
    mcpClientId:
      getOption(options, 'mcp-client-id')
      ?? env.BPANE_MCP_CLIENT_ID
      ?? null,
    mcpIssuer:
      getOption(options, 'mcp-issuer')
      ?? env.BPANE_MCP_ISSUER
      ?? null,
    mcpDisplayName:
      getOption(options, 'mcp-display-name')
      ?? env.BPANE_MCP_DISPLAY_NAME
      ?? null,
    fetchImpl,
  };
}

export async function runBpaneCli(argv, env = process.env, io = process, fetchImpl = globalThis.fetch) {
  const output = {
    stdout: io.stdout ?? process.stdout,
    stderr: io.stderr ?? process.stderr,
  };

  try {
    const { positionals, options } = parseArgs(argv);
    const wantsHelp = getOption(options, 'help') === 'true' || positionals[0] === 'help';
    if (wantsHelp) {
      output.stdout.write(`${usageText()}\n`);
      return EXIT_CODES.ok;
    }
    if (!positionals.length) {
      output.stderr.write(`${usageText()}\n`);
      return EXIT_CODES.usage;
    }
    if (typeof fetchImpl !== 'function') {
      throw new CliError('UNEXPECTED', 'No fetch implementation is available.', EXIT_CODES.unexpected);
    }

    const config = buildConfig(options, env, fetchImpl);
    const scope = positionals[0];
    let result;
    if (scope === 'session') {
      result = await handleSessionCommand(config, positionals);
    } else if (scope === 'mcp') {
      result = await handleMcpCommand(config, positionals);
    } else {
      throw new CliError('USAGE', `Unknown command scope: ${scope}`, EXIT_CODES.usage);
    }

    printJson(output, result);
    return EXIT_CODES.ok;
  } catch (error) {
    const payload = errorPayload(error);
    output.stderr.write(`${JSON.stringify(payload, null, 2)}\n`);
    return error instanceof CliError ? error.exitCode : EXIT_CODES.unexpected;
  }
}

const mainUrl = process.argv[1] ? pathToFileURL(process.argv[1]).href : '';
if (import.meta.url === mainUrl) {
  const code = await runBpaneCli(process.argv.slice(2));
  process.exitCode = code;
}
