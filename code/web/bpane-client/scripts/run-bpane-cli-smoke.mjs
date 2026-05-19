import process from 'node:process';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

import { chromium } from 'playwright-core';

import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  fetchAuthConfig,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

const log = createLogger('bpane-cli-smoke');

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-bpane-cli-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }

  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1360, height: 920 } });
  const page = await context.newPage();
  let accessToken = '';
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('Failed to acquire an admin access token.');
    }

    const bridge = await loadMcpBridgeConfig(options);
    sessionId = (await createSession(accessToken, options)).id;
    if (!sessionId) {
      throw new Error('Session creation did not return an id.');
    }

    const cliEnv = {
      ...process.env,
      BPANE_BASE_URL: apiOrigin(options),
      BPANE_ACCESS_TOKEN: accessToken,
      BPANE_MCP_CONTROL_URL: bridge.controlUrl,
      BPANE_MCP_CLIENT_ID: bridge.clientId,
      BPANE_MCP_ISSUER: bridge.issuer ?? '',
      BPANE_MCP_DISPLAY_NAME: bridge.displayName ?? '',
    };

    const listed = runBpaneCli(['session', 'list'], cliEnv);
    if (!Array.isArray(listed.sessions) || !listed.sessions.some((session) => session.id === sessionId)) {
      throw new Error(`CLI session list did not include ${sessionId}.`);
    }

    const fetched = runBpaneCli(['session', 'get', sessionId], cliEnv);
    if (fetched.id !== sessionId) {
      throw new Error(`CLI session get returned ${fetched.id ?? 'no id'} instead of ${sessionId}.`);
    }

    const status = runBpaneCli(['session', 'status', sessionId], cliEnv);
    if (!status.connection_counts) {
      throw new Error('CLI session status did not return connection_counts.');
    }

    const health = runBpaneCli(['mcp', 'health'], cliEnv);
    if (health.status !== 'ok') {
      throw new Error(`CLI MCP health returned ${health.status ?? 'no status'}.`);
    }

    const authorized = runBpaneCli(['mcp', 'authorize', sessionId], cliEnv);
    if (authorized.id !== sessionId) {
      throw new Error('CLI MCP authorize did not return the delegated session.');
    }

    const selected = runBpaneCli(['mcp', 'set-default', sessionId], cliEnv);
    if (selected.session?.id !== sessionId) {
      throw new Error('CLI MCP set-default did not return the selected session.');
    }

    const cleared = runBpaneCli(['mcp', 'clear-default'], cliEnv);
    if (cleared.ok !== true) {
      throw new Error('CLI MCP clear-default did not return ok=true.');
    }

    const revoked = runBpaneCli(['mcp', 'revoke', sessionId], cliEnv);
    if (revoked.id !== sessionId) {
      throw new Error('CLI MCP revoke did not return the session.');
    }

    const killed = runBpaneCli(['session', 'kill', sessionId], cliEnv);
    if (killed.id !== sessionId || killed.state !== 'stopped') {
      throw new Error('CLI session kill did not stop the session.');
    }
    sessionId = '';

    log('Operator CLI smoke passed.');
  } finally {
    if (sessionId && accessToken) {
      await clearMcpBridge(options).catch(() => {});
      await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/automation-owner`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
      await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/kill`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
    }
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function loadMcpBridgeConfig(options) {
  const config = await fetchAuthConfig({ ...options, pageUrl: apiOrigin(options) });
  const bridge = config?.mcpBridge;
  if (!bridge?.controlUrl || !bridge.clientId) {
    throw new Error('Operator CLI smoke requires auth-config mcpBridge metadata.');
  }
  return bridge;
}

async function createSession(accessToken, options) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      labels: {
        suite: 'bpane-cli-smoke',
      },
    }),
  });
}

async function clearMcpBridge(options) {
  const bridge = await loadMcpBridgeConfig(options);
  const response = await fetch(bridge.controlUrl, { method: 'DELETE' });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`Could not clear MCP bridge control session: HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

function runBpaneCli(args, env) {
  const cliPath = path.join(process.cwd(), 'scripts', 'bpane-cli.mjs');
  const result = spawnSync(process.execPath, [cliPath, ...args], {
    cwd: process.cwd(),
    env,
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const detail = result.stderr.trim() || result.stdout.trim() || result.error?.message || 'unknown error';
    throw new Error(`bpane CLI failed for ${args.join(' ')} with code ${result.status ?? 'unknown'}: ${detail}`);
  }
  const stdout = result.stdout.trim();
  if (!stdout) {
    return null;
  }
  try {
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(
      `bpane CLI returned invalid JSON for ${args.join(' ')}: ${
        error instanceof Error ? error.message : String(error)
      }; stdout length=${stdout.length}; tail=${stdout.slice(-240)}`,
    );
  }
}

run().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exit(1);
});
