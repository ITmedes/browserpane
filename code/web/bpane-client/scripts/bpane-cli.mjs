#!/usr/bin/env node

import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
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
    '  bpane profile init [profile-name] [options]',
    '  bpane profile list [options]',
    '  bpane profile show [profile-name] [options]',
    '  bpane session create [options]',
    '  bpane session list [options]',
    '  bpane session get <session-id> [options]',
    '  bpane session status <session-id> [options]',
    '  bpane session access-token <session-id> [options]',
    '  bpane session automation-access <session-id> [options]',
    '  bpane session disconnect-all <session-id> [options]',
    '  bpane session stop <session-id> [options]',
    '  bpane session kill <session-id> [options]',
    '  bpane session cleanup [options]',
    '  bpane mcp doctor [session-id] [options]',
    '  bpane mcp preflight [session-id] [options]',
    '  bpane mcp health [options]',
    '  bpane mcp authorize <session-id> [options]',
    '  bpane mcp revoke <session-id> [options]',
    '  bpane mcp set-default <session-id> [options]',
    '  bpane mcp clear-default [options]',
    '',
    'Options:',
    '  --config <path>          CLI config path. Env: BPANE_CONFIG. Default: ~/.config/bpane/config.json.',
    '  --profile <name>         CLI profile name. Env: BPANE_PROFILE. Defaults to config default_profile or default.',
    '  --set-default            Make profile init set the selected profile as default.',
    '  --save-token             Allow profile init to persist the provided access token.',
    '  --base-url <url>          Gateway/web origin. Env: BPANE_BASE_URL or BPANE_API_URL. Default: http://localhost:8080.',
    '  --access-token <token>    Bearer token. Env: BPANE_ACCESS_TOKEN.',
    '  --token <token>           Alias for --access-token.',
    '  --mcp-control-url <url>   MCP bridge control URL. Env: BPANE_MCP_CONTROL_URL.',
    '  --mcp-client-id <id>      MCP delegate client id. Env: BPANE_MCP_CLIENT_ID.',
    '  --mcp-issuer <issuer>     MCP delegate issuer. Env: BPANE_MCP_ISSUER.',
    '  --mcp-display-name <name> MCP delegate display name. Env: BPANE_MCP_DISPLAY_NAME.',
    '  --body-json <json>        Raw JSON request body for session create.',
    '  --label <key=value>       Repeatable session label filter or create label.',
    '  --state <state>           Repeatable cleanup state filter. Default: stopped.',
    '  --older-than-sec <sec>    Cleanup age filter based on created_at.',
    '  --confirm                 Execute cleanup. Without it cleanup is a dry-run.',
    '  --fail-on-issues          Make mcp doctor exit non-zero when diagnostics find issues.',
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

function getOptions(options, name) {
  return options.get(name) ?? [];
}

function optionEnabled(options, name) {
  const value = getOption(options, name, 'false');
  return ['1', 'true', 'yes', 'on'].includes(String(value).toLowerCase());
}

function parseJsonOption(options, name) {
  const raw = getOption(options, name);
  if (raw === null) {
    return null;
  }
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new CliError(
      'USAGE',
      `Invalid JSON for --${name}: ${error instanceof Error ? error.message : String(error)}`,
      EXIT_CODES.usage,
    );
  }
}

function parseIntegerOption(options, name) {
  const raw = getOption(options, name);
  if (raw === null || raw === '') {
    return null;
  }
  const value = Number.parseInt(raw, 10);
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new CliError('USAGE', `--${name} must be a positive integer.`, EXIT_CODES.usage);
  }
  return value;
}

function parseKeyValueOptions(options, name) {
  const parsed = {};
  for (const raw of getOptions(options, name)) {
    const separator = raw.indexOf('=');
    if (separator <= 0) {
      throw new CliError('USAGE', `--${name} must use key=value syntax.`, EXIT_CODES.usage);
    }
    const key = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1);
    if (!key) {
      throw new CliError('USAGE', `--${name} must not use an empty key.`, EXIT_CODES.usage);
    }
    parsed[key] = value;
  }
  return parsed;
}

function expandHome(filePath) {
  if (filePath === '~') {
    return os.homedir();
  }
  if (filePath.startsWith('~/')) {
    return path.join(os.homedir(), filePath.slice(2));
  }
  return filePath;
}

function defaultConfigPath() {
  return path.join(os.homedir(), '.config', 'bpane', 'config.json');
}

function configPath(options, env) {
  return expandHome(getOption(options, 'config') ?? env.BPANE_CONFIG ?? defaultConfigPath());
}

async function readCliConfig(options, env) {
  const filePath = configPath(options, env);
  try {
    const raw = await fs.readFile(filePath, 'utf8');
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new CliError('USAGE', `CLI config ${filePath} must contain a JSON object.`, EXIT_CODES.usage);
    }
    return {
      path: filePath,
      exists: true,
      config: parsed,
    };
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return {
        path: filePath,
        exists: false,
        config: {},
      };
    }
    if (error instanceof CliError) {
      throw error;
    }
    if (error instanceof SyntaxError) {
      throw new CliError('USAGE', `CLI config ${filePath} contains invalid JSON: ${error.message}`, EXIT_CODES.usage);
    }
    throw new CliError(
      'USAGE',
      `Failed to read CLI config ${filePath}: ${error instanceof Error ? error.message : String(error)}`,
      EXIT_CODES.usage,
    );
  }
}

function configProfiles(cliConfig) {
  const profiles = cliConfig.config.profiles ?? {};
  if (!profiles || typeof profiles !== 'object' || Array.isArray(profiles)) {
    throw new CliError('USAGE', `CLI config ${cliConfig.path} field profiles must be an object.`, EXIT_CODES.usage);
  }
  return profiles;
}

function resolveProfileName(options, env, cliConfig) {
  return getOption(options, 'profile')
    ?? env.BPANE_PROFILE
    ?? cliConfig.config.default_profile
    ?? cliConfig.config.defaultProfile
    ?? 'default';
}

function profileValue(profile, camelName, snakeName = null) {
  if (!profile || typeof profile !== 'object' || Array.isArray(profile)) {
    return null;
  }
  return profile[camelName] ?? (snakeName ? profile[snakeName] : undefined) ?? null;
}

function optionOrEnv(options, env, optionName, envName, fallback = null) {
  return getOption(options, optionName) ?? env[envName] ?? fallback;
}

function redactToken(value) {
  if (!value) {
    return '';
  }
  const token = String(value);
  if (token.length <= 8) {
    return '********';
  }
  return `${token.slice(0, 4)}...${token.slice(-4)}`;
}

function profileInitName(options, env, cliConfig, positionals) {
  if (positionals.length > 3) {
    throw new CliError('USAGE', 'Usage: bpane profile init [profile-name] [options]', EXIT_CODES.usage);
  }
  return positionals[2] ?? resolveProfileName(options, env, cliConfig);
}

function profileInitValues(options, env) {
  const values = {};
  const baseUrl = getOption(options, 'base-url') ?? getOption(options, 'api-url') ?? env.BPANE_BASE_URL ?? env.BPANE_API_URL ?? null;
  if (baseUrl) {
    values.base_url = normalizeBaseUrl(baseUrl);
  }
  const mcpControlUrl = optionOrEnv(options, env, 'mcp-control-url', 'BPANE_MCP_CONTROL_URL');
  if (mcpControlUrl) {
    values.mcp_control_url = mcpControlUrl;
  }
  const mcpClientId = optionOrEnv(options, env, 'mcp-client-id', 'BPANE_MCP_CLIENT_ID');
  if (mcpClientId) {
    values.mcp_client_id = mcpClientId;
  }
  const mcpIssuer = optionOrEnv(options, env, 'mcp-issuer', 'BPANE_MCP_ISSUER');
  if (mcpIssuer) {
    values.mcp_issuer = mcpIssuer;
  }
  const mcpDisplayName = optionOrEnv(options, env, 'mcp-display-name', 'BPANE_MCP_DISPLAY_NAME');
  if (mcpDisplayName) {
    values.mcp_display_name = mcpDisplayName;
  }
  const accessToken = getOption(options, 'access-token') ?? getOption(options, 'token') ?? env.BPANE_ACCESS_TOKEN ?? null;
  const tokenSaved = optionEnabled(options, 'save-token') && Boolean(accessToken);
  if (tokenSaved) {
    values.access_token = accessToken;
  }
  return {
    values,
    token_saved: tokenSaved,
    token_available: Boolean(accessToken),
  };
}

async function writeCliConfig(filePath, config) {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, `${JSON.stringify(config, null, 2)}\n`, { mode: 0o600 });
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

function commandResult(payload, exitCode = EXIT_CODES.ok) {
  return {
    __bpaneCliResult: true,
    payload,
    exitCode,
  };
}

function normalizeCommandResult(result) {
  if (result?.__bpaneCliResult === true) {
    return result;
  }
  return commandResult(result);
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

async function captureRequest(fn) {
  try {
    return { ok: true, body: await fn() };
  } catch (error) {
    return { ok: false, error: errorPayload(error) };
  }
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

function buildCreateSessionRequest(options) {
  const rawBody = parseJsonOption(options, 'body-json');
  if (rawBody !== null) {
    return rawBody;
  }

  const body = {};
  const templateId = getOption(options, 'template-id');
  if (templateId) {
    body.template_id = templateId;
  }
  const ownerMode = getOption(options, 'owner-mode');
  if (ownerMode) {
    body.owner_mode = ownerMode;
  }
  const width = parseIntegerOption(options, 'width');
  const height = parseIntegerOption(options, 'height');
  if ((width === null) !== (height === null)) {
    throw new CliError('USAGE', 'Use --width and --height together.', EXIT_CODES.usage);
  }
  if (width !== null && height !== null) {
    body.viewport = { width, height };
  }
  const idleTimeoutSec = parseIntegerOption(options, 'idle-timeout-sec');
  if (idleTimeoutSec !== null) {
    body.idle_timeout_sec = idleTimeoutSec;
  }
  const labels = parseKeyValueOptions(options, 'label');
  if (Object.keys(labels).length) {
    body.labels = labels;
  }
  const integrationContext = parseJsonOption(options, 'integration-json');
  if (integrationContext !== null) {
    body.integration_context = integrationContext;
  }
  const extensionIds = getOptions(options, 'extension-id').filter(Boolean);
  if (extensionIds.length) {
    body.extension_ids = extensionIds;
  }
  const recordingMode = getOption(options, 'recording-mode');
  const recordingRetentionSec = parseIntegerOption(options, 'recording-retention-sec');
  if (recordingMode || recordingRetentionSec !== null) {
    body.recording = {
      mode: recordingMode ?? 'disabled',
      format: 'webm',
      retention_sec: recordingRetentionSec,
    };
  }
  return body;
}

function cleanupStateFilters(options) {
  const states = getOptions(options, 'state')
    .flatMap((value) => value.split(','))
    .map((value) => value.trim())
    .filter(Boolean);
  return states.length ? states : ['stopped'];
}

function cleanupFilters(options) {
  const olderThanSec = parseIntegerOption(options, 'older-than-sec');
  const labels = parseKeyValueOptions(options, 'label');
  return {
    states: cleanupStateFilters(options),
    labels,
    older_than_sec: olderThanSec,
  };
}

function sessionCreatedBefore(session, olderThanSec) {
  if (olderThanSec === null) {
    return true;
  }
  const createdAt = Date.parse(session.created_at ?? '');
  return Number.isFinite(createdAt) && createdAt <= Date.now() - olderThanSec * 1000;
}

function sessionMatchesLabels(session, labels) {
  const sessionLabels = session.labels ?? {};
  return Object.entries(labels).every(([key, value]) => sessionLabels[key] === value);
}

function sessionSummary(session) {
  return {
    id: session.id,
    state: session.state,
    labels: session.labels ?? {},
    automation_delegate: session.automation_delegate ?? null,
    total_clients: session.status?.connection_counts?.total_clients ?? null,
    created_at: session.created_at ?? null,
    updated_at: session.updated_at ?? null,
  };
}

async function cleanupOperation(label, fn) {
  const result = await captureRequest(fn);
  if (result.ok) {
    return { operation: label, ok: true, response: result.body };
  }
  return { operation: label, ok: false, error: result.error };
}

async function cleanupSessions(config, options) {
  const filters = cleanupFilters(options);
  const confirmed = optionEnabled(options, 'confirm') && !optionEnabled(options, 'dry-run');
  const hasBoundingFilter =
    Object.keys(filters.labels).length > 0 || filters.older_than_sec !== null;
  if (confirmed && !hasBoundingFilter) {
    throw new CliError(
      'USAGE',
      'session cleanup --confirm requires at least one bounding --label or --older-than-sec filter.',
      EXIT_CODES.usage,
    );
  }

  const listed = await requestGateway(config, '/api/v1/sessions');
  const sessions = Array.isArray(listed?.sessions) ? listed.sessions : [];
  const candidates = sessions.filter((session) => {
    return filters.states.includes(session.state)
      && sessionMatchesLabels(session, filters.labels)
      && sessionCreatedBefore(session, filters.older_than_sec);
  });
  const actions = ['revoke-automation-owner', 'disconnect-all', 'kill'];

  if (!confirmed) {
    return {
      dry_run: true,
      filters,
      planned_actions: actions,
      candidate_count: candidates.length,
      candidates: candidates.map(sessionSummary),
    };
  }

  const results = [];
  for (const session of candidates) {
    const sessionId = session.id;
    const operations = [];
    operations.push(await cleanupOperation('revoke-automation-owner', async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
        { method: 'DELETE' },
      );
    }));
    operations.push(await cleanupOperation('disconnect-all', async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/connections/disconnect-all`,
        { method: 'POST' },
      );
    }));
    operations.push(await cleanupOperation('kill', async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`,
        { method: 'POST' },
      );
    }));
    results.push({
      session: sessionSummary(session),
      operations,
    });
  }

  return {
    dry_run: false,
    filters,
    result_count: results.length,
    results,
  };
}

function addDoctorIssue(issues, code, severity, message, remediation) {
  issues.push({ code, severity, message, remediation });
}

function controlSessionId(controlBody) {
  return controlBody?.session?.id ?? null;
}

function managedHealthEntry(healthBody, sessionId) {
  const entries = Array.isArray(healthBody?.managed_sessions) ? healthBody.managed_sessions : [];
  return entries.find((entry) => entry?.session_id === sessionId) ?? null;
}

async function runMcpDoctor(config, sessionId) {
  const mcpConfig = await resolveMcpConfig(config, { control: true, client: true });
  const issues = [];
  const bridge = {
    control_url: mcpConfig.controlUrl,
    health_url: bridgeHealthUrl(mcpConfig.controlUrl),
    client_id: mcpConfig.clientId,
  };

  const health = await captureRequest(async () => {
    return await requestJson(config, bridge.health_url);
  });
  if (!health.ok) {
    addDoctorIssue(
      issues,
      'MCP_BRIDGE_HEALTH_UNREACHABLE',
      'error',
      'The MCP bridge health endpoint is not reachable.',
      'Start the local mcp-bridge service or pass --mcp-control-url for the intended bridge.',
    );
  }

  const control = await captureRequest(async () => {
    return await requestJson(config, mcpConfig.controlUrl, { method: 'GET' });
  });
  if (!control.ok) {
    addDoctorIssue(
      issues,
      'MCP_CONTROL_SESSION_UNREACHABLE',
      'error',
      'The MCP bridge control-session endpoint is not reachable.',
      'Check the mcp-bridge container and the configured control URL.',
    );
  }

  const sessionChecks = sessionId
    ? {
        requested_session_id: sessionId,
      }
    : null;

  if (sessionId) {
    if (!config.accessToken) {
      addDoctorIssue(
        issues,
        'AUTH_REQUIRED',
        'error',
        'Session-specific MCP diagnostics require a BrowserPane access token.',
        'Pass --access-token/--token or set BPANE_ACCESS_TOKEN.',
      );
    } else {
      const session = await captureRequest(async () => {
        return await requestGateway(config, `/api/v1/sessions/${encodeURIComponent(sessionId)}`);
      });
      const status = await captureRequest(async () => {
        return await requestGateway(config, `/api/v1/sessions/${encodeURIComponent(sessionId)}/status`);
      });

      sessionChecks.session = session;
      sessionChecks.status = status;

      if (!session.ok) {
        addDoctorIssue(
          issues,
          'SESSION_NOT_VISIBLE',
          'error',
          `Session ${sessionId} is not visible to the current owner token.`,
          'Check the session id and token owner before delegating MCP.',
        );
      } else {
        const resource = session.body;
        const delegate = resource?.automation_delegate ?? null;
        sessionChecks.state = resource?.state ?? null;
        sessionChecks.automation_delegate = delegate;

        if (resource?.state === 'stopped') {
          addDoctorIssue(
            issues,
            'SESSION_STOPPED',
            'warning',
            `Session ${sessionId} is stopped.`,
            'Start or reconnect the session before using MCP automation against a live browser.',
          );
        }

        if (!delegate) {
          addDoctorIssue(
            issues,
            'MCP_DELEGATE_MISSING',
            'warning',
            `Session ${sessionId} is not delegated to the MCP bridge client.`,
            `Run bpane mcp authorize ${sessionId}.`,
          );
        } else if (delegate.client_id !== mcpConfig.clientId) {
          addDoctorIssue(
            issues,
            'MCP_DELEGATE_MISMATCH',
            'error',
            `Session ${sessionId} is delegated to ${delegate.client_id}, not ${mcpConfig.clientId}.`,
            `Run bpane mcp authorize ${sessionId} with the intended --mcp-client-id.`,
          );
        }
      }

      if (status.ok) {
        sessionChecks.mcp_owner = status.body?.mcp_owner ?? null;
      }
    }

    if (control.ok) {
      const selectedSessionId = controlSessionId(control.body);
      sessionChecks.bridge_default_session_id = selectedSessionId;
      if (selectedSessionId !== sessionId) {
        addDoctorIssue(
          issues,
          'MCP_DEFAULT_SESSION_MISMATCH',
          'warning',
          selectedSessionId
            ? `The MCP bridge default session is ${selectedSessionId}, not ${sessionId}.`
            : 'The MCP bridge has no default session selected.',
          `Run bpane mcp set-default ${sessionId}.`,
        );
      }
    }

    if (health.ok) {
      sessionChecks.bridge_health_entry = managedHealthEntry(health.body, sessionId);
    }
  }

  return {
    ok: issues.length === 0,
    bridge,
    control_session: control.ok ? control.body : control.error,
    health: health.ok ? health.body : health.error,
    session: sessionChecks,
    issues,
  };
}

async function handleSessionCommand(config, positionals, options) {
  const action = positionals[1];
  if (action === 'create' && positionals.length === 2) {
    return await requestGateway(config, '/api/v1/sessions', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildCreateSessionRequest(options)),
    });
  }
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
  if (action === 'access-token') {
    const sessionId = requiredSessionId(positionals, 'session access-token');
    return await requestGateway(
      config,
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/access-tokens`,
      { method: 'POST' },
    );
  }
  if (action === 'automation-access') {
    const sessionId = requiredSessionId(positionals, 'session automation-access');
    return await requestGateway(
      config,
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-access`,
      { method: 'POST' },
    );
  }
  if (action === 'disconnect-all') {
    const sessionId = requiredSessionId(positionals, 'session disconnect-all');
    return await requestGateway(
      config,
      `/api/v1/sessions/${encodeURIComponent(sessionId)}/connections/disconnect-all`,
      { method: 'POST' },
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
  if (action === 'cleanup' && positionals.length === 2) {
    return await cleanupSessions(config, options);
  }
  throw new CliError('USAGE', `Unknown session command: ${action ?? ''}`.trim(), EXIT_CODES.usage);
}

async function handleMcpCommand(config, positionals, options) {
  const action = positionals[1];
  if ((action === 'doctor' || action === 'preflight') && positionals.length <= 3) {
    const diagnostics = await runMcpDoctor(config, positionals[2] ?? null);
    const strict = action === 'preflight' || optionEnabled(options, 'fail-on-issues');
    return commandResult(
      diagnostics,
      strict && !diagnostics.ok ? EXIT_CODES.api : EXIT_CODES.ok,
    );
  }
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

async function handleProfileCommand(options, env, positionals) {
  const action = positionals[1];
  const cliConfig = await readCliConfig(options, env);
  const profiles = configProfiles(cliConfig);
  if (action === 'init' && positionals.length <= 3) {
    const profileName = profileInitName(options, env, cliConfig, positionals);
    const existingProfile = profiles[profileName] ?? {};
    const init = profileInitValues(options, env);
    const nextConfig = {
      ...cliConfig.config,
      profiles: {
        ...profiles,
        [profileName]: {
          ...existingProfile,
          ...init.values,
        },
      },
    };
    if (optionEnabled(options, 'set-default') || !nextConfig.default_profile) {
      nextConfig.default_profile = profileName;
    }
    await writeCliConfig(cliConfig.path, nextConfig);
    return {
      config_path: cliConfig.path,
      profile: profileName,
      created: !profiles[profileName],
      default_profile: nextConfig.default_profile,
      token_saved: init.token_saved,
      token_available: init.token_available,
      values: {
        base_url: nextConfig.profiles[profileName].base_url ?? nextConfig.profiles[profileName].baseUrl ?? null,
        access_token: redactToken(nextConfig.profiles[profileName].access_token ?? nextConfig.profiles[profileName].accessToken),
        mcp_control_url: nextConfig.profiles[profileName].mcp_control_url ?? nextConfig.profiles[profileName].mcpControlUrl ?? null,
        mcp_client_id: nextConfig.profiles[profileName].mcp_client_id ?? nextConfig.profiles[profileName].mcpClientId ?? null,
        mcp_issuer: nextConfig.profiles[profileName].mcp_issuer ?? nextConfig.profiles[profileName].mcpIssuer ?? null,
        mcp_display_name: nextConfig.profiles[profileName].mcp_display_name ?? nextConfig.profiles[profileName].mcpDisplayName ?? null,
      },
    };
  }
  if (action === 'list' && positionals.length === 2) {
    const activeProfile = resolveProfileName(options, env, cliConfig);
    return {
      config_path: cliConfig.path,
      config_exists: cliConfig.exists,
      active_profile: activeProfile,
      profiles: Object.keys(profiles).sort(),
    };
  }
  if (action === 'show' && positionals.length <= 3) {
    const profileName = positionals[2] ?? resolveProfileName(options, env, cliConfig);
    const profile = selectedProfile(cliConfig, profileName) ?? {};
    return {
      config_path: cliConfig.path,
      config_exists: cliConfig.exists,
      profile: profileName,
      values: {
        base_url: profileValue(profile, 'baseUrl', 'base_url') ?? null,
        access_token: redactToken(profileValue(profile, 'accessToken', 'access_token')),
        mcp_control_url: profileValue(profile, 'mcpControlUrl', 'mcp_control_url') ?? null,
        mcp_client_id: profileValue(profile, 'mcpClientId', 'mcp_client_id') ?? null,
        mcp_issuer: profileValue(profile, 'mcpIssuer', 'mcp_issuer') ?? null,
        mcp_display_name: profileValue(profile, 'mcpDisplayName', 'mcp_display_name') ?? null,
      },
    };
  }
  throw new CliError('USAGE', `Unknown profile command: ${action ?? ''}`.trim(), EXIT_CODES.usage);
}

function selectedProfile(cliConfig, profileName) {
  const profiles = configProfiles(cliConfig);
  const profile = profiles[profileName] ?? null;
  const hasProfiles = Object.keys(profiles).length > 0;
  if (profile) {
    return profile;
  }
  if (cliConfig.exists && hasProfiles) {
    throw new CliError(
      'USAGE',
      `CLI profile ${profileName} was not found in ${cliConfig.path}.`,
      EXIT_CODES.usage,
    );
  }
  return cliConfig.exists && !hasProfiles ? cliConfig.config : null;
}

async function buildConfig(options, env, fetchImpl) {
  const cliConfig = await readCliConfig(options, env);
  const profileName = resolveProfileName(options, env, cliConfig);
  const profile = selectedProfile(cliConfig, profileName);
  return {
    profileName,
    cliConfigPath: cliConfig.path,
    cliConfigExists: cliConfig.exists,
    baseUrl: normalizeBaseUrl(
      getOption(options, 'base-url')
      ?? getOption(options, 'api-url')
      ?? env.BPANE_BASE_URL
      ?? env.BPANE_API_URL
      ?? profileValue(profile, 'baseUrl', 'base_url')
      ?? 'http://localhost:8080',
    ),
    accessToken:
      getOption(options, 'access-token')
      ?? getOption(options, 'token')
      ?? env.BPANE_ACCESS_TOKEN
      ?? profileValue(profile, 'accessToken', 'access_token')
      ?? '',
    mcpControlUrl:
      getOption(options, 'mcp-control-url')
      ?? env.BPANE_MCP_CONTROL_URL
      ?? profileValue(profile, 'mcpControlUrl', 'mcp_control_url')
      ?? null,
    mcpClientId:
      getOption(options, 'mcp-client-id')
      ?? env.BPANE_MCP_CLIENT_ID
      ?? profileValue(profile, 'mcpClientId', 'mcp_client_id')
      ?? null,
    mcpIssuer:
      getOption(options, 'mcp-issuer')
      ?? env.BPANE_MCP_ISSUER
      ?? profileValue(profile, 'mcpIssuer', 'mcp_issuer')
      ?? null,
    mcpDisplayName:
      getOption(options, 'mcp-display-name')
      ?? env.BPANE_MCP_DISPLAY_NAME
      ?? profileValue(profile, 'mcpDisplayName', 'mcp_display_name')
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

    const scope = positionals[0];
    let result;
    if (scope === 'profile') {
      result = await handleProfileCommand(options, env, positionals);
    } else if (scope === 'session') {
      const config = await buildConfig(options, env, fetchImpl);
      result = await handleSessionCommand(config, positionals, options);
    } else if (scope === 'mcp') {
      const config = await buildConfig(options, env, fetchImpl);
      result = await handleMcpCommand(config, positionals, options);
    } else {
      throw new CliError('USAGE', `Unknown command scope: ${scope}`, EXIT_CODES.usage);
    }

    const command = normalizeCommandResult(result);
    printJson(output, command.payload);
    return command.exitCode;
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
