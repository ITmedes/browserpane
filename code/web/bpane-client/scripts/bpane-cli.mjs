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

const KNOWN_OPTIONS = new Set([
  'access-token',
  'api-url',
  'base-url',
  'body-json',
  'browser-context-id',
  'browser-context-mode',
  'browser-identity',
  'bypass-rule',
  'cleanup-action',
  'config',
  'confirm',
  'custom-ca-name',
  'custom-ca-ref',
  'default-label',
  'description',
  'dry-run',
  'egress-profile-id',
  'extension-id',
  'fail-on-issues',
  'geolocation-accuracy-meters',
  'geolocation-latitude',
  'geolocation-longitude',
  'height',
  'help',
  'idle-timeout-sec',
  'input',
  'integration-json',
  'integration',
  'label',
  'language',
  'limit',
  'locale',
  'mcp-client-id',
  'mcp-control-url',
  'mcp-display-name',
  'mcp-issuer',
  'max-profile-storage-bytes',
  'name',
  'older-than-sec',
  'offset',
  'output',
  'owner-mode',
  'profile',
  'proxy-url',
  'persistence-mode',
  'recording-mode',
  'recording-retention-sec',
  'retention-sec',
  'save-token',
  'set-default',
  'runtime-state',
  'state',
  'template-id',
  'token',
  'timezone',
  'traffic-observation-mode',
  'sensitive-log-sink-name',
  'sensitive-log-sink-ref',
  'user-agent',
  'width',
]);

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
    '  bpane session-template create [template-name] [options]',
    '  bpane session-template list [options]',
    '  bpane session-template get <template-id> [options]',
    '  bpane session-template update <template-id> [options]',
    '  bpane egress-profile create [profile-name] [options]',
    '  bpane egress-profile list [options]',
    '  bpane egress-profile get <profile-id> [options]',
    '  bpane browser-context create [context-name] [options]',
    '  bpane browser-context clone <source-context-id> <target-context-name> [options]',
    '  bpane browser-context export <context-id> --output <path> [options]',
    '  bpane browser-context import --input <zip> --name <target-context-name> [options]',
    '  bpane browser-context list [options]',
    '  bpane browser-context get <context-id> [options]',
    '  bpane browser-context delete <context-id> [options]',
    '  bpane mcp doctor [session-id] [options]',
    '  bpane mcp preflight [session-id] [options]',
    '  bpane mcp repair <session-id> [options]',
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
    '  --api-url <url>           Alias for --base-url.',
    '  --access-token <token>    Bearer token. Env: BPANE_ACCESS_TOKEN.',
    '  --token <token>           Alias for --access-token.',
    '  --mcp-control-url <url>   MCP bridge control URL. Env: BPANE_MCP_CONTROL_URL.',
    '  --mcp-client-id <id>      MCP delegate client id. Env: BPANE_MCP_CLIENT_ID.',
    '  --mcp-issuer <issuer>     MCP delegate issuer. Env: BPANE_MCP_ISSUER.',
    '  --mcp-display-name <name> MCP delegate display name. Env: BPANE_MCP_DISPLAY_NAME.',
    '  --body-json <json>        Raw JSON request body for session create.',
    '  --label <key=value>       Repeatable session label filter or create label.',
    '  --default-label <key=value> Repeatable template default session label.',
    '  --integration <key=value> Repeatable session integration-context filter.',
    '  --state <state>           Repeatable cleanup state filter. Default: stopped.',
    '  --runtime-state <state>   Repeatable session runtime-state filter.',
    '  --template-id <id>        Session template id for create/list filters.',
    '  --browser-context-id <id> Browser context id for reusable session creation.',
    '  --browser-context-mode <mode> Browser context mode: fresh, ephemeral, reusable.',
    '  --locale <tag>            Session locale, for example de-DE.',
    '  --language <tag>          Repeatable session language preference.',
    '  --timezone <zone>         Session timezone, for example Europe/Berlin.',
    '  --geolocation-latitude <number>  Session geolocation latitude.',
    '  --geolocation-longitude <number> Session geolocation longitude.',
    '  --geolocation-accuracy-meters <number> Optional geolocation accuracy.',
    '  --user-agent <value>      Custom Chromium user agent for session create/template defaults.',
    '  --browser-identity <id>   Approved browser identity hint for session create/template defaults.',
    '  --egress-profile-id <id>  Approved egress profile id for session create/template defaults.',
    '  --proxy-url <url>         Egress profile proxy URL.',
    '  --bypass-rule <rule>      Repeatable egress profile proxy bypass rule.',
    '  --custom-ca-ref <ref>     Egress profile custom CA reference.',
    '  --custom-ca-name <name>   Egress profile custom CA display name.',
    '  --traffic-observation-mode <mode> Egress observation mode: metadata_only or tls_intercept.',
    '  --sensitive-log-sink-ref <ref> Approved SIEM/log-sink ref required for tls_intercept.',
    '  --sensitive-log-sink-name <name> Sensitive log-sink display name.',
    '  --name <name>             Session template name for create/update.',
    '  --description <text>      Session template description for create/update.',
    '  --persistence-mode <mode> Browser context persistence mode. Default: reusable.',
    '  --retention-sec <sec>     Browser context retention window in seconds.',
  '  --max-profile-storage-bytes <bytes> Browser context profile storage limit in bytes.',
  '  --input <path>            File path for binary import/upload commands.',
  '  --output <path>           File path for binary export/download commands.',
    '  --cleanup-action <name>   Repeatable cleanup action: revoke-automation-owner, disconnect-all, stop, kill.',
    '  --older-than-sec <sec>    Cleanup age filter based on created_at.',
    '  --limit <count>           Limit filtered session list or cleanup candidates.',
    '  --confirm                 Execute cleanup. Without it cleanup is a dry-run.',
    '  --dry-run                 Force cleanup preview mode even when --confirm is present.',
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

    const equalsIndex = token.indexOf('=');
    const rawName = equalsIndex === -1 ? token : token.slice(0, equalsIndex);
    const inlineValue = equalsIndex === -1 ? undefined : token.slice(equalsIndex + 1);
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

function validateOptions(options) {
  const unknown = Array.from(options.keys()).filter((name) => !KNOWN_OPTIONS.has(name));
  if (unknown.length) {
    throw new CliError(
      'USAGE',
      `Unsupported option${unknown.length === 1 ? '' : 's'}: ${unknown.map((name) => `--${name}`).join(', ')}`,
      EXIT_CODES.usage,
    );
  }
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

function parseNonNegativeIntegerOption(options, name) {
  const raw = getOption(options, name);
  if (raw === null || raw === '') {
    return null;
  }
  const value = Number.parseInt(raw, 10);
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new CliError('USAGE', `--${name} must be a non-negative integer.`, EXIT_CODES.usage);
  }
  return value;
}

function parseFiniteNumberOption(options, name) {
  const raw = getOption(options, name);
  if (raw === null || raw === '') {
    return null;
  }
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    throw new CliError('USAGE', `--${name} must be a finite number.`, EXIT_CODES.usage);
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
  await fs.chmod(filePath, 0o600);
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

function requiredTemplateId(positionals, commandLabel) {
  const templateId = positionals[2];
  if (!templateId || positionals.length > 3) {
    throw new CliError('USAGE', `Usage: bpane ${commandLabel} <template-id>`, EXIT_CODES.usage);
  }
  return templateId;
}

function requiredBrowserContextId(positionals, commandLabel) {
  const contextId = positionals[2];
  if (!contextId || positionals.length > 3) {
    throw new CliError('USAGE', `Usage: bpane ${commandLabel} <context-id>`, EXIT_CODES.usage);
  }
  return contextId;
}

function requiredEgressProfileId(positionals, commandLabel) {
  const profileId = positionals[2];
  if (!profileId || positionals.length > 3) {
    throw new CliError('USAGE', `Usage: bpane ${commandLabel} <profile-id>`, EXIT_CODES.usage);
  }
  return profileId;
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

async function requestGatewayBinary(config, path, init = {}) {
  const headers = jsonHeaders(config, init.headers);
  let response;
  const url = joinUrl(config.baseUrl, path);
  try {
    response = await config.fetchImpl(url, {
      ...init,
      headers,
    });
  } catch (error) {
    throw new CliError(
      'REQUEST_FAILED',
      error instanceof Error ? error.message : String(error),
      EXIT_CODES.api,
      { url },
    );
  }
  const buffer = Buffer.from(await response.arrayBuffer());
  if (!response.ok) {
    throw new CliError(
      'HTTP_ERROR',
      `HTTP ${response.status}${buffer.length ? ` ${buffer.toString('utf8')}` : ''}`,
      EXIT_CODES.api,
      { status: response.status, body: buffer.toString('utf8') },
    );
  }
  return {
    bytes: buffer,
    contentType: response.headers?.get?.('content-type') ?? null,
  };
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

function buildNetworkIdentityRequest(options) {
  const identity = {};
  const locale = getOption(options, 'locale');
  if (locale) {
    identity.locale = locale;
  }
  const languages = getOptions(options, 'language')
    .flatMap((value) => value.split(','))
    .map((value) => value.trim())
    .filter(Boolean);
  if (languages.length) {
    identity.languages = Array.from(new Set(languages));
  }
  const timezone = getOption(options, 'timezone');
  if (timezone) {
    identity.timezone = timezone;
  }
  const userAgent = getOption(options, 'user-agent');
  if (userAgent) {
    if (/[\r\n]/u.test(userAgent)) {
      throw new CliError('USAGE', '--user-agent must be a single line.', EXIT_CODES.usage);
    }
    identity.user_agent = userAgent;
  }
  const browserIdentity = getOption(options, 'browser-identity');
  if (browserIdentity) {
    identity.browser_identity = browserIdentity;
  }
  const egressProfileId = getOption(options, 'egress-profile-id');
  if (egressProfileId) {
    identity.egress_profile_id = egressProfileId;
  }
  const latitude = parseFiniteNumberOption(options, 'geolocation-latitude');
  const longitude = parseFiniteNumberOption(options, 'geolocation-longitude');
  const accuracyMeters = parseFiniteNumberOption(options, 'geolocation-accuracy-meters');
  if (latitude !== null || longitude !== null || accuracyMeters !== null) {
    if (latitude === null || longitude === null) {
      throw new CliError(
        'USAGE',
        'Use --geolocation-latitude and --geolocation-longitude together.',
        EXIT_CODES.usage,
      );
    }
    if (latitude < -90 || latitude > 90) {
      throw new CliError('USAGE', '--geolocation-latitude must be between -90 and 90.', EXIT_CODES.usage);
    }
    if (longitude < -180 || longitude > 180) {
      throw new CliError('USAGE', '--geolocation-longitude must be between -180 and 180.', EXIT_CODES.usage);
    }
    if (accuracyMeters !== null && accuracyMeters <= 0) {
      throw new CliError('USAGE', '--geolocation-accuracy-meters must be greater than zero.', EXIT_CODES.usage);
    }
    identity.geolocation = {
      latitude,
      longitude,
      ...(accuracyMeters !== null ? { accuracy_meters: accuracyMeters } : {}),
    };
  }
  return Object.keys(identity).length ? identity : null;
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
  const browserContextId = getOption(options, 'browser-context-id');
  const browserContextMode = getOption(options, 'browser-context-mode');
  if (browserContextId || browserContextMode) {
    const mode = browserContextMode ?? 'reusable';
    if (mode === 'reusable' && !browserContextId) {
      throw new CliError(
        'USAGE',
        '--browser-context-id is required when --browser-context-mode is reusable.',
        EXIT_CODES.usage,
      );
    }
    if (mode !== 'reusable' && browserContextId) {
      throw new CliError(
        'USAGE',
        '--browser-context-id can only be used with reusable browser contexts.',
        EXIT_CODES.usage,
      );
    }
    body.browser_context = { mode };
    if (browserContextId) {
      body.browser_context.context_id = browserContextId;
    }
  }
  const ownerMode = getOption(options, 'owner-mode');
  if (ownerMode) {
    body.owner_mode = ownerMode;
  }
  const networkIdentity = buildNetworkIdentityRequest(options);
  if (networkIdentity !== null) {
    body.network_identity = networkIdentity;
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

function buildSessionTemplateDefaults(options) {
  const defaults = {};
  const ownerMode = getOption(options, 'owner-mode');
  if (ownerMode) {
    defaults.owner_mode = ownerMode;
  }
  const width = parseIntegerOption(options, 'width');
  const height = parseIntegerOption(options, 'height');
  if ((width === null) !== (height === null)) {
    throw new CliError('USAGE', 'Use --width and --height together.', EXIT_CODES.usage);
  }
  if (width !== null && height !== null) {
    defaults.viewport = { width, height };
  }
  const idleTimeoutSec = parseIntegerOption(options, 'idle-timeout-sec');
  if (idleTimeoutSec !== null) {
    defaults.idle_timeout_sec = idleTimeoutSec;
  }
  const networkIdentity = buildNetworkIdentityRequest(options);
  if (networkIdentity !== null) {
    defaults.network_identity = networkIdentity;
  }
  const labels = parseKeyValueOptions(options, 'default-label');
  if (Object.keys(labels).length) {
    defaults.labels = labels;
  }
  const integrationContext = parseJsonOption(options, 'integration-json');
  if (integrationContext !== null) {
    defaults.integration_context = integrationContext;
  }
  const recordingMode = getOption(options, 'recording-mode');
  const recordingRetentionSec = parseIntegerOption(options, 'recording-retention-sec');
  if (recordingMode || recordingRetentionSec !== null) {
    defaults.recording = {
      mode: recordingMode ?? 'disabled',
      format: 'webm',
      retention_sec: recordingRetentionSec,
    };
  }
  return defaults;
}

function isObjectRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function mergeOptionalObjectDefaults(existingValue, overrideValue) {
  if (isObjectRecord(existingValue) && isObjectRecord(overrideValue)) {
    return { ...existingValue, ...overrideValue };
  }
  return overrideValue ?? existingValue;
}

function mergeSessionTemplateNetworkIdentity(existingValue, overrideValue) {
  if (!isObjectRecord(existingValue)) {
    return overrideValue;
  }
  if (!isObjectRecord(overrideValue)) {
    return existingValue;
  }
  return {
    ...existingValue,
    ...overrideValue,
  };
}

function mergeSessionTemplateDefaults(existingDefaults, overrideDefaults) {
  const existing = isObjectRecord(existingDefaults) ? existingDefaults : {};
  const overrides = isObjectRecord(overrideDefaults) ? overrideDefaults : {};
  const merged = { ...existing };

  for (const key of ['owner_mode', 'viewport', 'idle_timeout_sec', 'recording']) {
    if (Object.prototype.hasOwnProperty.call(overrides, key)) {
      merged[key] = overrides[key];
    }
  }
  if (Object.prototype.hasOwnProperty.call(overrides, 'labels')) {
    merged.labels = {
      ...(isObjectRecord(existing.labels) ? existing.labels : {}),
      ...(isObjectRecord(overrides.labels) ? overrides.labels : {}),
    };
  } else if (isObjectRecord(existing.labels)) {
    merged.labels = existing.labels;
  }
  if (Object.prototype.hasOwnProperty.call(overrides, 'integration_context')) {
    merged.integration_context = mergeOptionalObjectDefaults(
      existing.integration_context,
      overrides.integration_context,
    );
  } else if (Object.prototype.hasOwnProperty.call(existing, 'integration_context')) {
    merged.integration_context = existing.integration_context;
  }
  if (Object.prototype.hasOwnProperty.call(overrides, 'network_identity')) {
    merged.network_identity = mergeSessionTemplateNetworkIdentity(
      existing.network_identity,
      overrides.network_identity,
    );
  } else if (Object.prototype.hasOwnProperty.call(existing, 'network_identity')) {
    merged.network_identity = existing.network_identity;
  }

  return merged;
}

function buildSessionTemplateRequest(options, fallbackName = null) {
  const rawBody = parseJsonOption(options, 'body-json');
  if (rawBody !== null) {
    return rawBody;
  }

  const name = getOption(options, 'name') ?? fallbackName;
  if (!name) {
    throw new CliError(
      'USAGE',
      'Session template create/update requires --name or a positional template name.',
      EXIT_CODES.usage,
    );
  }
  const body = {
    name,
    defaults: buildSessionTemplateDefaults(options),
  };
  const description = getOption(options, 'description');
  if (description !== null) {
    body.description = description;
  }
  const labels = parseKeyValueOptions(options, 'label');
  if (Object.keys(labels).length) {
    body.labels = labels;
  }
  return body;
}

function buildSessionTemplateUpdateRequest(existingTemplate, options) {
  const rawBody = parseJsonOption(options, 'body-json');
  if (rawBody !== null) {
    return rawBody;
  }

  const name = getOption(options, 'name') ?? existingTemplate?.name;
  if (!name) {
    throw new CliError(
      'USAGE',
      'Session template update requires --name when the existing template response has no name.',
      EXIT_CODES.usage,
    );
  }
  const body = {
    name,
    labels: {
      ...(isObjectRecord(existingTemplate?.labels) ? existingTemplate.labels : {}),
      ...parseKeyValueOptions(options, 'label'),
    },
    defaults: mergeSessionTemplateDefaults(
      existingTemplate?.defaults ?? {},
      buildSessionTemplateDefaults(options),
    ),
  };
  const description = getOption(options, 'description');
  if (description !== null) {
    body.description = description;
  } else if (existingTemplate?.description != null) {
    body.description = existingTemplate.description;
  }
  return body;
}

function buildEgressProfileRequest(options, fallbackName = null) {
  const rawBody = parseJsonOption(options, 'body-json');
  if (rawBody !== null) {
    return rawBody;
  }

  const name = getOption(options, 'name') ?? fallbackName;
  if (!name) {
    throw new CliError(
      'USAGE',
      'Egress profile create requires --name or a positional profile name.',
      EXIT_CODES.usage,
    );
  }
  const body = { name };
  const description = getOption(options, 'description');
  if (description !== null) {
    body.description = description;
  }
  const labels = parseKeyValueOptions(options, 'label');
  if (Object.keys(labels).length) {
    body.labels = labels;
  }
  const proxyUrl = getOption(options, 'proxy-url');
  if (proxyUrl) {
    body.proxy = { url: proxyUrl };
  }
  const bypassRules = getOptions(options, 'bypass-rule')
    .flatMap((value) => value.split(','))
    .map((value) => value.trim())
    .filter(Boolean);
  if (bypassRules.length) {
    body.bypass_rules = Array.from(new Set(bypassRules));
  }
  const customCaRef = getOption(options, 'custom-ca-ref');
  const customCaName = getOption(options, 'custom-ca-name');
  if (customCaRef || customCaName) {
    if (!customCaRef) {
      throw new CliError('USAGE', '--custom-ca-name requires --custom-ca-ref.', EXIT_CODES.usage);
    }
    body.custom_ca = {
      certificate_ref: customCaRef,
      ...(customCaName ? { display_name: customCaName } : {}),
    };
  }
  const trafficObservationMode = getOption(options, 'traffic-observation-mode');
  const sensitiveLogSinkRef = getOption(options, 'sensitive-log-sink-ref');
  const sensitiveLogSinkName = getOption(options, 'sensitive-log-sink-name');
  if (trafficObservationMode || sensitiveLogSinkRef || sensitiveLogSinkName) {
    const mode = trafficObservationMode ?? 'metadata_only';
    if (!['metadata_only', 'tls_intercept'].includes(mode)) {
      throw new CliError(
        'USAGE',
        '--traffic-observation-mode must be metadata_only or tls_intercept.',
        EXIT_CODES.usage,
      );
    }
    if (sensitiveLogSinkName && !sensitiveLogSinkRef) {
      throw new CliError('USAGE', '--sensitive-log-sink-name requires --sensitive-log-sink-ref.', EXIT_CODES.usage);
    }
    if (mode === 'tls_intercept') {
      if (!body.proxy) {
        throw new CliError('USAGE', '--traffic-observation-mode tls_intercept requires --proxy-url.', EXIT_CODES.usage);
      }
      if (!body.custom_ca) {
        throw new CliError('USAGE', '--traffic-observation-mode tls_intercept requires --custom-ca-ref.', EXIT_CODES.usage);
      }
      if (!sensitiveLogSinkRef) {
        throw new CliError('USAGE', '--traffic-observation-mode tls_intercept requires --sensitive-log-sink-ref.', EXIT_CODES.usage);
      }
    }
    body.traffic_observation = {
      mode,
      ...(sensitiveLogSinkRef ? { sensitive_log_sink_ref: sensitiveLogSinkRef } : {}),
      ...(sensitiveLogSinkName ? { sensitive_log_sink_display_name: sensitiveLogSinkName } : {}),
    };
  }
  const state = getOption(options, 'state');
  if (state) {
    body.state = state;
  }
  return body;
}

function buildBrowserContextRequest(options, fallbackName = null, commandLabel = 'create') {
  const rawBody = parseJsonOption(options, 'body-json');
  if (rawBody !== null) {
    return rawBody;
  }

  const name = getOption(options, 'name') ?? fallbackName;
  if (!name) {
    throw new CliError(
      'USAGE',
      `Browser context ${commandLabel} requires --name or a positional context name.`,
      EXIT_CODES.usage,
    );
  }
  const body = { name };
  const description = getOption(options, 'description');
  if (description !== null) {
    body.description = description;
  }
  const labels = parseKeyValueOptions(options, 'label');
  if (Object.keys(labels).length) {
    body.labels = labels;
  }
  const persistenceMode = getOption(options, 'persistence-mode');
  if (persistenceMode) {
    body.persistence_mode = persistenceMode;
  }
  const retentionSec = parseIntegerOption(options, 'retention-sec');
  if (retentionSec !== null) {
    body.retention_sec = retentionSec;
  }
  const maxProfileStorageBytes = parseIntegerOption(options, 'max-profile-storage-bytes');
  if (maxProfileStorageBytes !== null) {
    body.max_profile_storage_bytes = maxProfileStorageBytes;
  }
  return body;
}

function sessionStateFilters(options, defaultStates = []) {
  const states = getOptions(options, 'state')
    .flatMap((value) => value.split(','))
    .map((value) => value.trim())
    .filter(Boolean);
  return states.length ? states : defaultStates;
}

function sessionRuntimeStateFilters(options) {
  return getOptions(options, 'runtime-state')
    .flatMap((value) => value.split(','))
    .map((value) => value.trim())
    .filter(Boolean);
}

function sessionFilters(options, defaultStates = []) {
  const olderThanSec = parseIntegerOption(options, 'older-than-sec');
  const limit = parseIntegerOption(options, 'limit');
  const offset = parseNonNegativeIntegerOption(options, 'offset');
  const labels = parseKeyValueOptions(options, 'label');
  const integrationContext = parseKeyValueOptions(options, 'integration');
  return {
    template_id: getOption(options, 'template-id'),
    states: sessionStateFilters(options, defaultStates),
    runtime_states: sessionRuntimeStateFilters(options),
    labels,
    integration_context: integrationContext,
    older_than_sec: olderThanSec,
    limit,
    offset,
  };
}

function cleanupFilters(options) {
  return sessionFilters(options, ['stopped']);
}

function filtersAreActive(filters) {
  return filters.template_id !== null
    || filters.states.length > 0
    || filters.runtime_states.length > 0
    || Object.keys(filters.labels).length > 0
    || Object.keys(filters.integration_context).length > 0
    || filters.older_than_sec !== null
    || filters.limit !== null
    || filters.offset !== null;
}

function cleanupActions(options) {
  const requested = getOptions(options, 'cleanup-action')
    .flatMap((value) => value.split(','))
    .map((value) => value.trim())
    .filter(Boolean);
  const actions = requested.length
    ? requested
    : ['revoke-automation-owner', 'disconnect-all', 'kill'];
  const allowed = new Set(['revoke-automation-owner', 'disconnect-all', 'stop', 'kill']);
  for (const action of actions) {
    if (!allowed.has(action)) {
      throw new CliError(
        'USAGE',
        `Unsupported cleanup action ${action}. Supported actions: ${Array.from(allowed).join(', ')}.`,
        EXIT_CODES.usage,
      );
    }
  }
  return Array.from(new Set(actions));
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

function sessionMatchesIntegrationContext(session, integrationContext) {
  const context = session.integration_context ?? {};
  return Object.entries(integrationContext).every(([key, value]) => context[key] === value);
}

function filterSessions(sessions, filters) {
  const matched = sessions.filter((session) => {
    return (filters.template_id === null || session.template_id === filters.template_id)
      && (!filters.states.length || filters.states.includes(session.state))
      && (!filters.runtime_states.length || filters.runtime_states.includes(session.status?.runtime_state))
      && sessionMatchesLabels(session, filters.labels)
      && sessionMatchesIntegrationContext(session, filters.integration_context)
      && sessionCreatedBefore(session, filters.older_than_sec);
  });
  const offset = filters.offset ?? 0;
  const limited = filters.limit === null
    ? matched.slice(offset)
    : matched.slice(offset, offset + filters.limit);
  return { matched, limited };
}

function buildSessionListPath(filters, { includeLimit = true } = {}) {
  const params = new URLSearchParams();
  if (filters.template_id) {
    params.set('template_id', filters.template_id);
  }
  if (filters.states.length) {
    params.set('state', filters.states.join(','));
  }
  if (filters.runtime_states.length) {
    params.set('runtime_state', filters.runtime_states.join(','));
  }
  for (const [key, value] of Object.entries(filters.labels)) {
    params.append(`label.${key}`, value);
  }
  for (const [key, value] of Object.entries(filters.integration_context)) {
    params.append(`integration.${key}`, value);
  }
  if (includeLimit && filters.limit !== null) {
    params.set('limit', String(filters.limit));
  }
  if (includeLimit && filters.offset !== null) {
    params.set('offset', String(filters.offset));
  }
  const query = params.toString();
  return query ? `/api/v1/sessions?${query}` : '/api/v1/sessions';
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

function filteredSessionList(listed, filters) {
  const sessions = Array.isArray(listed?.sessions) ? listed.sessions : [];
  if (!filtersAreActive(filters)) {
    return listed;
  }
  const { matched, limited } = filterSessions(sessions, filters);
  return {
    filters,
    total_count: sessions.length,
    matched_count: matched.length,
    returned_count: limited.length,
    sessions: limited,
  };
}

async function cleanupOperation(label, fn) {
  const result = await captureRequest(fn);
  if (result.ok) {
    return { operation: label, ok: true, response: result.body };
  }
  return { operation: label, ok: false, error: result.error };
}

async function executeCleanupAction(config, sessionId, action) {
  if (action === 'revoke-automation-owner') {
    return await cleanupOperation(action, async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
        { method: 'DELETE' },
      );
    });
  }
  if (action === 'disconnect-all') {
    return await cleanupOperation(action, async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/connections/disconnect-all`,
        { method: 'POST' },
      );
    });
  }
  if (action === 'stop') {
    return await cleanupOperation(action, async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/stop`,
        { method: 'POST' },
      );
    });
  }
  if (action === 'kill') {
    return await cleanupOperation(action, async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`,
        { method: 'POST' },
      );
    });
  }
  throw new CliError('USAGE', `Unsupported cleanup action ${action}.`, EXIT_CODES.usage);
}

async function cleanupSessions(config, options) {
  const filters = cleanupFilters(options);
  const actions = cleanupActions(options);
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
  const { matched, limited: candidates } = filterSessions(sessions, filters);
  if (!confirmed) {
    return {
      dry_run: true,
      filters,
      planned_actions: actions,
      candidate_count: candidates.length,
      matched_count: matched.length,
      total_count: sessions.length,
      candidates: candidates.map(sessionSummary),
    };
  }

  const results = [];
  let failureCount = 0;
  for (const session of candidates) {
    const sessionId = session.id;
    const operations = [];
    for (const action of actions) {
      const operation = await executeCleanupAction(config, sessionId, action);
      if (!operation.ok) {
        failureCount += 1;
      }
      operations.push(operation);
    }
    results.push({
      session: sessionSummary(session),
      operations,
    });
  }

  return commandResult({
    dry_run: false,
    filters,
    planned_actions: actions,
    result_count: results.length,
    matched_count: matched.length,
    total_count: sessions.length,
    failure_count: failureCount,
    results,
  }, failureCount > 0 ? EXIT_CODES.api : EXIT_CODES.ok);
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

function repairBlockingIssues(diagnostics) {
  const blockingCodes = new Set(['AUTH_REQUIRED', 'SESSION_NOT_VISIBLE']);
  return diagnostics.issues.filter((issue) => blockingCodes.has(issue.code));
}

function blockedRepairAction(action) {
  return {
    action,
    attempted: false,
    ok: false,
    blocked: true,
    reason: 'MCP repair requires the session to be visible to the current owner token before mutating delegation.',
  };
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

async function repairMcpDelegation(config, sessionId) {
  const mcpConfig = await resolveMcpConfig(config, { control: true, client: true });
  const actions = [];
  let failureCount = 0;

  const initial = await runMcpDoctor(config, sessionId);
  const blockingIssues = repairBlockingIssues(initial);
  if (blockingIssues.length > 0) {
    return commandResult({
      ok: false,
      session_id: sessionId,
      blocked: true,
      blocking_issues: blockingIssues,
      failure_count: 0,
      actions: [
        blockedRepairAction('authorize'),
        blockedRepairAction('set-default'),
      ],
      diagnostics: initial,
    }, EXIT_CODES.api);
  }

  const needsDelegate = initial.issues.some((issue) => {
    return issue.code === 'MCP_DELEGATE_MISSING' || issue.code === 'MCP_DELEGATE_MISMATCH';
  });
  if (needsDelegate) {
    const result = await captureRequest(async () => {
      return await requestGateway(
        config,
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(buildDelegateBody(mcpConfig)),
        },
      );
    });
    if (!result.ok) {
      failureCount += 1;
    }
    actions.push({
      action: 'authorize',
      attempted: true,
      ok: result.ok,
      response: result.ok ? result.body : undefined,
      error: result.ok ? undefined : result.error,
    });
  } else {
    actions.push({
      action: 'authorize',
      attempted: false,
      ok: true,
      reason: 'session already delegated to the configured MCP client',
    });
  }

  const needsDefault = initial.issues.some((issue) => issue.code === 'MCP_DEFAULT_SESSION_MISMATCH');
  if (needsDefault) {
    const result = await captureRequest(async () => {
      return await requestJson(config, mcpConfig.controlUrl, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: sessionId }),
      });
    });
    if (!result.ok) {
      failureCount += 1;
    }
    actions.push({
      action: 'set-default',
      attempted: true,
      ok: result.ok,
      response: result.ok ? result.body : undefined,
      error: result.ok ? undefined : result.error,
    });
  } else {
    actions.push({
      action: 'set-default',
      attempted: false,
      ok: true,
      reason: 'MCP bridge default session already matches',
    });
  }

  const diagnostics = await runMcpDoctor(config, sessionId);
  return commandResult({
    ok: failureCount === 0 && diagnostics.ok,
    session_id: sessionId,
    failure_count: failureCount,
    actions,
    diagnostics,
  }, failureCount > 0 || !diagnostics.ok ? EXIT_CODES.api : EXIT_CODES.ok);
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
    const filters = sessionFilters(options);
    const listed = await requestGateway(
      config,
      buildSessionListPath(filters, { includeLimit: filters.older_than_sec === null }),
    );
    if (filters.older_than_sec !== null) {
      return filteredSessionList(listed, filters);
    }
    return listed;
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

async function handleSessionTemplateCommand(config, positionals, options) {
  const action = positionals[1];
  if (action === 'create' && positionals.length <= 3) {
    return await requestGateway(config, '/api/v1/session-templates', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildSessionTemplateRequest(options, positionals[2] ?? null)),
    });
  }
  if (action === 'list' && positionals.length === 2) {
    return await requestGateway(config, '/api/v1/session-templates');
  }
  if (action === 'get') {
    const templateId = requiredTemplateId(positionals, 'session-template get');
    return await requestGateway(config, `/api/v1/session-templates/${encodeURIComponent(templateId)}`);
  }
  if (action === 'update') {
    const templateId = requiredTemplateId(positionals, 'session-template update');
    const existingTemplate = await requestGateway(
      config,
      `/api/v1/session-templates/${encodeURIComponent(templateId)}`,
    );
    return await requestGateway(config, `/api/v1/session-templates/${encodeURIComponent(templateId)}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildSessionTemplateUpdateRequest(existingTemplate, options)),
    });
  }
  throw new CliError(
    'USAGE',
    `Unknown session-template command: ${action ?? ''}`.trim(),
    EXIT_CODES.usage,
  );
}

async function handleEgressProfileCommand(config, positionals, options) {
  const action = positionals[1];
  if (action === 'create' && positionals.length <= 3) {
    return await requestGateway(config, '/api/v1/egress-profiles', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildEgressProfileRequest(options, positionals[2] ?? null)),
    });
  }
  if (action === 'list' && positionals.length === 2) {
    return await requestGateway(config, '/api/v1/egress-profiles');
  }
  if (action === 'get') {
    const profileId = requiredEgressProfileId(positionals, 'egress-profile get');
    return await requestGateway(config, `/api/v1/egress-profiles/${encodeURIComponent(profileId)}`);
  }
  throw new CliError(
    'USAGE',
    `Unknown egress-profile command: ${action ?? ''}`.trim(),
    EXIT_CODES.usage,
  );
}

async function handleBrowserContextCommand(config, positionals, options) {
  const action = positionals[1];
  if (action === 'create' && positionals.length <= 3) {
    return await requestGateway(config, '/api/v1/browser-contexts', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildBrowserContextRequest(options, positionals[2] ?? null)),
    });
  }
  if (action === 'list' && positionals.length === 2) {
    return await requestGateway(config, '/api/v1/browser-contexts');
  }
  if (action === 'clone' && positionals.length <= 4) {
    const contextId = positionals[2];
    if (!contextId) {
      throw new CliError('USAGE', 'browser-context clone requires a context id.', EXIT_CODES.usage);
    }
    const targetName = positionals[3] ?? null;
    return await requestGateway(config, `/api/v1/browser-contexts/${encodeURIComponent(contextId)}/clone`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildBrowserContextRequest(options, targetName, 'clone')),
    });
  }
  if (action === 'export' && positionals.length === 3) {
    const contextId = requiredBrowserContextId(positionals, 'browser-context export');
    const outputPath = getOption(options, 'output');
    if (!outputPath) {
      throw new CliError('USAGE', 'browser-context export requires --output <path>.', EXIT_CODES.usage);
    }
    const { bytes, contentType } = await requestGatewayBinary(
      config,
      `/api/v1/browser-contexts/${encodeURIComponent(contextId)}/export`,
      {
        method: 'GET',
        headers: { Accept: 'application/zip' },
      },
    );
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, bytes);
    return {
      context_id: contextId,
      output_path: outputPath,
      byte_count: bytes.length,
      content_type: contentType,
    };
  }
  if (action === 'import' && positionals.length === 2) {
    const inputPath = getOption(options, 'input');
    if (!inputPath) {
      throw new CliError('USAGE', 'browser-context import requires --input <path>.', EXIT_CODES.usage);
    }
    const name = getOption(options, 'name');
    if (!name) {
      throw new CliError('USAGE', 'browser-context import requires --name <target-context-name>.', EXIT_CODES.usage);
    }
    const archive = await fs.readFile(inputPath);
    const headers = {
      Accept: 'application/json',
      'Content-Type': 'application/zip',
      'x-bpane-browser-context-name': name,
    };
    const description = getOption(options, 'description');
    if (description !== null) {
      headers['x-bpane-browser-context-description'] = description;
    }
    const labels = parseKeyValueOptions(options, 'label');
    if (Object.keys(labels).length) {
      headers['x-bpane-browser-context-labels'] = JSON.stringify(labels);
    }
    const retentionSec = parseIntegerOption(options, 'retention-sec');
    if (retentionSec !== null) {
      headers['x-bpane-browser-context-retention-sec'] = String(retentionSec);
    }
    const maxProfileStorageBytes = parseIntegerOption(options, 'max-profile-storage-bytes');
    if (maxProfileStorageBytes !== null) {
      headers['x-bpane-browser-context-max-profile-storage-bytes'] = String(maxProfileStorageBytes);
    }
    return await requestGateway(config, '/api/v1/browser-contexts/import', {
      method: 'POST',
      headers,
      body: archive,
    });
  }
  if (action === 'get') {
    const contextId = requiredBrowserContextId(positionals, 'browser-context get');
    return await requestGateway(config, `/api/v1/browser-contexts/${encodeURIComponent(contextId)}`);
  }
  if (action === 'delete') {
    const contextId = requiredBrowserContextId(positionals, 'browser-context delete');
    return await requestGateway(config, `/api/v1/browser-contexts/${encodeURIComponent(contextId)}`, {
      method: 'DELETE',
    });
  }
  throw new CliError(
    'USAGE',
    `Unknown browser-context command: ${action ?? ''}`.trim(),
    EXIT_CODES.usage,
  );
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
  if (action === 'repair') {
    const sessionId = requiredSessionId(positionals, 'mcp repair');
    return await repairMcpDelegation(config, sessionId);
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
    validateOptions(options);
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
    } else if (scope === 'session-template') {
      const config = await buildConfig(options, env, fetchImpl);
      result = await handleSessionTemplateCommand(config, positionals, options);
    } else if (scope === 'egress-profile') {
      const config = await buildConfig(options, env, fetchImpl);
      result = await handleEgressProfileCommand(config, positionals, options);
    } else if (scope === 'browser-context') {
      const config = await buildConfig(options, env, fetchImpl);
      result = await handleBrowserContextCommand(config, positionals, options);
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
