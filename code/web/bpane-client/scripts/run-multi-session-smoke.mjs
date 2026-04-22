import fs from 'node:fs/promises';
import { execFile } from 'node:child_process';
import process from 'node:process';
import { promisify } from 'node:util';
import { chromium } from 'playwright-core';

const execFileAsync = promisify(execFile);

const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  mcpHealthUrl: 'http://localhost:8931/health',
  mcpRegisterUrl: 'http://localhost:8931/register',
  mcpControlUrl: 'http://localhost:8931/control-session',
  certSpki: process.env.BPANE_BENCHMARK_CERT_SPKI ?? '',
  connectTimeoutMs: 30000,
  headless: false,
  outputPath: '',
};

const COMMON_CHROME_PATHS = [
  process.env.BPANE_BENCHMARK_CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].filter(Boolean);

const SESSION_ID_RE =
  /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;

function parseArgs(argv) {
  const options = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--page-url' && next) {
      options.pageUrl = next;
      i++;
    } else if (arg === '--mcp-health-url' && next) {
      options.mcpHealthUrl = next;
      i++;
    } else if (arg === '--mcp-register-url' && next) {
      options.mcpRegisterUrl = next;
      i++;
    } else if (arg === '--mcp-control-url' && next) {
      options.mcpControlUrl = next;
      i++;
    } else if (arg === '--cert-spki' && next) {
      options.certSpki = next;
      i++;
    } else if (arg === '--connect-timeout-ms' && next) {
      options.connectTimeoutMs = Number(next);
      i++;
    } else if (arg === '--output' && next) {
      options.outputPath = next;
      i++;
    } else if (arg === '--headless') {
      options.headless = true;
    } else if (arg === '--help') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function printHelp() {
  console.log(`
Usage: node scripts/run-multi-session-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --mcp-health-url <url>      MCP bridge health URL (default: ${DEFAULTS.mcpHealthUrl})
  --mcp-register-url <url>    MCP bridge register URL (default: ${DEFAULTS.mcpRegisterUrl})
  --mcp-control-url <url>     MCP bridge control URL (default: ${DEFAULTS.mcpControlUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[multi-session-smoke] ${message}`);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function poll(description, fn, predicate, timeoutMs, intervalMs = 500) {
  const startedAt = Date.now();
  let lastValue = null;
  while (Date.now() - startedAt < timeoutMs) {
    lastValue = await fn();
    if (predicate(lastValue)) {
      return lastValue;
    }
    await sleep(intervalMs);
  }
  throw new Error(`Timed out waiting for ${description}`);
}

async function resolveChromeExecutable() {
  for (const path of COMMON_CHROME_PATHS) {
    try {
      await fs.access(path);
      return path;
    } catch {
      // ignore
    }
  }
  throw new Error(
    'No Chrome/Chromium executable found. Set BPANE_BENCHMARK_CHROME to a local Chrome path.',
  );
}

async function resolveCertSpki(options) {
  if (options.certSpki?.trim()) {
    return options.certSpki.trim();
  }
  try {
    const value = await fs.readFile(
      new URL('../../../../dev/certs/cert-fingerprint.txt', import.meta.url),
      'utf8',
    );
    return value.trim();
  } catch {
    return '';
  }
}

async function fetchAuthConfig(options) {
  try {
    const response = await fetch(new URL('/auth-config.json', options.pageUrl));
    if (!response.ok) {
      return null;
    }
    return await response.json();
  } catch {
    return null;
  }
}

async function configurePage(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => Boolean(window.__bpaneAuth));
}

async function ensureLoggedIn(page, options) {
  const authConfig = await fetchAuthConfig(options);
  if (!authConfig || authConfig.mode !== 'oidc') {
    return authConfig;
  }

  const state = await page.evaluate(() => ({
    configured: window.__bpaneAuth?.isConfigured?.() ?? false,
    authenticated: window.__bpaneAuth?.isAuthenticated?.() ?? false,
    exampleUser: window.__bpaneAuth?.getExampleUser?.() ?? null,
  }));
  if (!state.configured || state.authenticated) {
    return authConfig;
  }

  const exampleUser = state.exampleUser;
  if (!exampleUser?.username || !exampleUser?.password) {
    throw new Error('OIDC auth is enabled, but no example user is configured for smoke login.');
  }

  await page.click('#btn-login');
  const username = page.locator('input[name="username"], #username').first();
  const password = page.locator('input[name="password"], #password').first();
  const loginState = await poll(
    'OIDC login readiness',
    async () => ({
      authenticated: await page
        .evaluate(() => window.__bpaneAuth?.isAuthenticated?.() === true)
        .catch(() => false),
      usernameVisible: await username.isVisible().catch(() => false),
      currentUrl: page.url(),
    }),
    (state) => state.authenticated || state.usernameVisible,
    options.connectTimeoutMs,
  );
  if (!loginState.authenticated) {
    await username.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await password.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await username.fill(exampleUser.username);
    await password.fill(exampleUser.password);
    await page.locator('input[type="submit"], #kc-login').click();
  }

  const pageUrl = new URL(options.pageUrl);
  const targetPrefix = `${pageUrl.origin}${pageUrl.pathname}`;
  await page.waitForURL(new RegExp(`^${targetPrefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`), {
    timeout: options.connectTimeoutMs,
  });
  await page.waitForFunction(() => window.__bpaneAuth?.isAuthenticated?.() === true, {
    timeout: options.connectTimeoutMs,
  });

  return authConfig;
}

async function getAccessToken(page) {
  return await page.evaluate(() => window.__bpaneAuth?.getAccessToken?.() ?? null);
}

async function getDisplayedSessionId(page) {
  const text = (await page.locator('#session-display').textContent()) ?? '';
  return text.match(SESSION_ID_RE)?.[0] ?? '';
}

async function getSelectedSessionId(page) {
  const value = await page.locator('#session-select').inputValue().catch(() => '');
  return SESSION_ID_RE.test(value) ? value : '';
}

async function getCurrentSessionId(page) {
  return (await getDisplayedSessionId(page)) || (await getSelectedSessionId(page));
}

async function refreshSessions(page) {
  await page.click('#btn-refresh-sessions');
  await page.waitForTimeout(400);
}

async function startNewSession(page, options) {
  await page.click('#btn-new-session');
  await page.waitForFunction(
    () => document.querySelector('#status')?.textContent?.trim() === 'Connected',
    { timeout: options.connectTimeoutMs },
  );
  await page.waitForSelector('#desktop-container canvas', { timeout: options.connectTimeoutMs });
  const sessionId = await poll(
    'session id after new-session connect',
    () => getCurrentSessionId(page),
    (value) => Boolean(value),
    options.connectTimeoutMs,
  );
  if (!sessionId) {
    throw new Error('Session creation completed without a visible session id.');
  }
  return sessionId;
}

async function selectSession(page, sessionId) {
  await page.selectOption('#session-select', sessionId);
  await page.waitForFunction(
    (expected) => document.querySelector('#session-select')?.value === expected,
    sessionId,
  );
}

async function connectSession(page, options) {
  await page.click('#btn-connect');
  await page.waitForFunction(
    () => document.querySelector('#status')?.textContent?.trim() === 'Connected',
    { timeout: options.connectTimeoutMs },
  );
  await page.waitForSelector('#desktop-container canvas', { timeout: options.connectTimeoutMs });
  await page.waitForFunction(
    () => document.querySelector('#resolution')?.textContent?.includes('x'),
    { timeout: options.connectTimeoutMs },
  );
}

async function disconnectSession(page) {
  const status = ((await page.locator('#status').textContent()) ?? '').trim();
  if (status !== 'Disconnected') {
    await page.click('#btn-disconnect');
    await page.waitForFunction(
      () => document.querySelector('#status')?.textContent?.trim() === 'Disconnected',
      { timeout: 15000 },
    );
  }
}

async function delegateMcp(page, sessionId, options) {
  await page.click('#btn-delegate-mcp');
  return await poll(
    `MCP bridge control session ${sessionId}`,
    () => fetchJson(options.mcpHealthUrl),
    (payload) => payload?.control_session_id === sessionId,
    options.connectTimeoutMs,
  );
}

async function fetchJson(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return await response.json();
}

async function registerMcp(options) {
  return await fetchJson(options.mcpRegisterUrl, { method: 'POST' });
}

async function clearBridgeControl(options) {
  try {
    await fetchJson(options.mcpControlUrl, { method: 'DELETE' });
  } catch {
    // ignore cleanup failures
  }
}

function containerNameForSession(sessionId) {
  return `bpane-runtime-${sessionId.replaceAll('-', '')}`;
}

async function lookupRuntimeContainerId(sessionId) {
  const containerName = containerNameForSession(sessionId);
  const { stdout } = await execFileAsync('docker', [
    'ps',
    '-q',
    '--filter',
    `name=^/${containerName}$`,
  ]);
  return stdout.trim();
}

async function fetchSessionResource(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchSessionStatus(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/status`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function clearAutomationOwner(accessToken, options, sessionId) {
  try {
    await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/automation-owner`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${accessToken}` },
    });
  } catch (error) {
    log(
      `cleanup warning: failed to clear automation delegate for ${sessionId}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function deleteSession(accessToken, options, sessionId) {
  try {
    const response = await fetch(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    if (!response.ok && response.status !== 404) {
      const detail = await response.text().catch(() => '');
      throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
    }
  } catch (error) {
    log(
      `cleanup warning: failed to stop session ${sessionId}: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const executablePath = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);
  const chromeArgs = [
    '--origin-to-force-quic-on=localhost:4433',
    '--disable-background-timer-throttling',
    '--disable-renderer-backgrounding',
    '--disable-backgrounding-occluded-windows',
  ];
  if (certSpki) {
    chromeArgs.push(`--ignore-certificate-errors-spki-list=${certSpki}`);
  }

  const browser = await chromium.launch({
    headless: options.headless,
    executablePath,
    args: chromeArgs,
  });

  let context = null;
  let pageOwnerA = null;
  let pageOwnerB = null;
  let pageViewer = null;
  let accessToken = '';
  const createdSessions = [];

  try {
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });

    pageOwnerA = await context.newPage();
    await configurePage(pageOwnerA, options);
    await ensureLoggedIn(pageOwnerA, options);
    accessToken = (await getAccessToken(pageOwnerA)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    pageOwnerB = await context.newPage();
    await configurePage(pageOwnerB, options);
    await ensureLoggedIn(pageOwnerB, options);

    pageViewer = await context.newPage();
    await configurePage(pageViewer, options);
    await ensureLoggedIn(pageViewer, options);

    log('Starting session A');
    const sessionA = await startNewSession(pageOwnerA, options);
    createdSessions.push(sessionA);

    log('Starting session B');
    const sessionB = await startNewSession(pageOwnerB, options);
    createdSessions.push(sessionB);

    if (sessionA === sessionB) {
      throw new Error(`Expected distinct sessions, but both pages resolved to ${sessionA}`);
    }

    log(`Joining existing session ${sessionA} from a third page`);
    await refreshSessions(pageViewer);
    await selectSession(pageViewer, sessionA);
    await connectSession(pageViewer, options);
    const joinedSession = await getCurrentSessionId(pageViewer);
    if (joinedSession !== sessionA) {
      throw new Error(`Expected viewer page to join ${sessionA}, got ${joinedSession || 'none'}`);
    }

    const containerA = await lookupRuntimeContainerId(sessionA);
    const containerB = await lookupRuntimeContainerId(sessionB);
    if (!containerA || !containerB) {
      throw new Error('Expected both session runtimes to have active worker containers.');
    }

    const sessionAResource = await fetchSessionResource(accessToken, options, sessionA);
    const sessionBResource = await fetchSessionResource(accessToken, options, sessionB);
    const sessionAStatus = await fetchSessionStatus(accessToken, options, sessionA);
    const sessionBStatus = await fetchSessionStatus(accessToken, options, sessionB);

    log(`Delegating MCP to session ${sessionA}`);
    const healthAfterA = await delegateMcp(pageOwnerA, sessionA, options);
    const registerAfterA = await registerMcp(options);
    const healthAfterRegisterA = await fetchJson(options.mcpHealthUrl);

    log(`Switching MCP to session ${sessionB}`);
    const healthAfterB = await delegateMcp(pageOwnerB, sessionB, options);
    const registerAfterB = await registerMcp(options);
    const healthAfterRegisterB = await fetchJson(options.mcpHealthUrl);

    const summary = {
      scenario: 'multi-session-compose-smoke',
      pageUrl: options.pageUrl,
      sessions: {
        ownerA: {
          id: sessionA,
          worker_container_id: containerA,
          runtime: sessionAResource.runtime,
          status: sessionAStatus,
        },
        ownerB: {
          id: sessionB,
          worker_container_id: containerB,
          runtime: sessionBResource.runtime,
          status: sessionBStatus,
        },
        viewer: {
          joined_session_id: joinedSession,
        },
      },
      bridge: {
        after_delegate_a: healthAfterA,
        after_register_a: registerAfterA,
        after_delegate_b: healthAfterB,
        after_register_b: registerAfterB,
        final_health: healthAfterRegisterB,
      },
    };

    if (sessionAStatus.browser_clients < 2) {
      throw new Error(
        `Expected session ${sessionA} to have at least 2 browser clients, got ${sessionAStatus.browser_clients}`,
      );
    }
    if (healthAfterRegisterA.control_session_id !== sessionA) {
      throw new Error('MCP bridge did not adopt session A as the control session.');
    }
    if (healthAfterRegisterA.playwright_cdp_endpoint !== sessionAResource.runtime.cdp_endpoint) {
      throw new Error('MCP bridge Playwright endpoint did not match session A runtime metadata.');
    }
    if (healthAfterRegisterB.control_session_id !== sessionB) {
      throw new Error('MCP bridge did not switch to session B as the control session.');
    }
    if (healthAfterRegisterB.playwright_cdp_endpoint !== sessionBResource.runtime.cdp_endpoint) {
      throw new Error('MCP bridge Playwright endpoint did not match session B runtime metadata.');
    }

    log(`Verified two parallel sessions (${sessionA}, ${sessionB}) and MCP bridge switching.`);
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
  } finally {
    if (pageViewer) {
      await disconnectSession(pageViewer).catch(() => {});
    }
    if (pageOwnerB) {
      await disconnectSession(pageOwnerB).catch(() => {});
    }
    if (pageOwnerA) {
      await disconnectSession(pageOwnerA).catch(() => {});
    }
    let cleanupAccessToken = accessToken;
    if (pageOwnerA) {
      cleanupAccessToken = ((await getAccessToken(pageOwnerA).catch(() => cleanupAccessToken)) ??
        cleanupAccessToken);
    }
    if (cleanupAccessToken) {
      await clearBridgeControl(options);
      for (const sessionId of createdSessions) {
        await clearAutomationOwner(cleanupAccessToken, options, sessionId);
        await deleteSession(cleanupAccessToken, options, sessionId);
        await poll(
          `runtime container cleanup for ${sessionId}`,
          () => lookupRuntimeContainerId(sessionId),
          (containerId) => !containerId,
          15000,
        ).catch((error) => {
          log(
            `cleanup warning: runtime container for ${sessionId} is still present after stop: ${error instanceof Error ? error.message : String(error)}`,
          );
        });
      }
    }
    if (context) {
      await context.close().catch(() => {});
    }
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(`[multi-session-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`);
  process.exitCode = 1;
});
