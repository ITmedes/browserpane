import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupWorkflowSmokeSessions,
  createLogger,
  fetchAuthConfig,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  testEmbedPageUrl,
} from './workflow-smoke-lib.mjs';

async function configureEmbedPage(page, options) {
  await page.goto(testEmbedPageUrl(options), { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(
    () => Boolean(window.__bpaneAuth && window.__bpaneControl),
    { timeout: options.connectTimeoutMs },
  );
}

async function ensureEmbedLoggedIn(page, options) {
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

  await page.locator('#btn-login').click();
  const username = page.locator('input[name="username"], #username').first();
  const password = page.locator('input[name="password"], #password').first();
  const loginState = await poll(
    'OIDC login readiness',
    async () => ({
      authenticated: await page
        .evaluate(() => window.__bpaneAuth?.isAuthenticated?.() === true)
        .catch(() => false),
      usernameVisible: await username.isVisible().catch(() => false),
    }),
    (value) => value.authenticated || value.usernameVisible,
    options.connectTimeoutMs,
  );
  if (!loginState.authenticated) {
    await username.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await password.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await username.fill(exampleUser.username);
    await password.fill(exampleUser.password);
    await page.locator('input[type="submit"], #kc-login').click();
  }

  const pageUrl = new URL(testEmbedPageUrl(options));
  const targetPrefix = `${pageUrl.origin}${pageUrl.pathname}`;
  await page.waitForURL(new RegExp(`^${targetPrefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`), {
    timeout: options.connectTimeoutMs,
  });
  await page.waitForFunction(() => window.__bpaneAuth?.isAuthenticated?.() === true, {
    timeout: options.connectTimeoutMs,
  });
  return authConfig;
}

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-test-embed-lifecycle-smoke.mjs');
  const log = createLogger('test-embed-lifecycle-smoke');

  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();

  try {
    log(`Opening ${testEmbedPageUrl(options)}`);
    await configureEmbedPage(page, options);
    await ensureEmbedLoggedIn(page, options);

    const accessToken = await getAccessToken(page);
    if (accessToken) {
      await cleanupWorkflowSmokeSessions(accessToken, options, log);
      await page.evaluate(async () => {
        await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
      });
    }

    log('Starting a new session from the standard layout.');
    await page.click('#btn-new-session');

    const connected = await poll(
      'connected session lifecycle state',
      async () => await page.evaluate(async () => {
        const status = await window.__bpaneControl.refreshSessionStatus({ force: true, silent: true });
        const stopButton = document.getElementById('btn-session-stop');
        const disconnectAllButton = document.getElementById('btn-session-disconnect-all');
        return {
          control: window.__bpaneControl.getState(),
          status,
          stopDisabled: stopButton?.disabled ?? true,
          disconnectAllDisabled: disconnectAllButton?.disabled ?? true,
          stopEligibilityText: document.getElementById('session-stop-eligibility')?.textContent?.trim() ?? '',
        };
      }),
      (value) => value.control?.connected === true && value.status?.connection_counts?.total_clients >= 1,
      options.connectTimeoutMs,
    );

    if (!connected.control?.sessionId) {
      throw new Error('Expected a connected session id after starting a new session.');
    }
    if (connected.status?.stop_eligibility?.allowed !== false) {
      throw new Error('Expected safe stop to be blocked while the owner is still connected.');
    }
    if (!connected.stopDisabled) {
      throw new Error('Expected Stop Selected to stay disabled while the current connection is still attached.');
    }
    if (connected.disconnectAllDisabled) {
      throw new Error('Expected Disconnect All to remain enabled while live attachments exist.');
    }

    const summary = {
      pageUrl: options.pageUrl,
      sessionId: connected.control.sessionId,
      connected: connected.control.connected,
      stopDisabled: connected.stopDisabled,
      disconnectAllDisabled: connected.disconnectAllDisabled,
      stopEligibility: connected.status?.stop_eligibility ?? null,
      connectionCounts: connected.status?.connection_counts ?? null,
      stopEligibilityText: connected.stopEligibilityText,
    };
    console.log(JSON.stringify(summary, null, 2));

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    try {
      await page.evaluate(async () => {
        const state = window.__bpaneControl?.getState?.();
        if (state?.connected) {
          await window.__bpaneControl.disconnect();
        }
      });
    } catch {
      // Ignore cleanup failures.
    }
    try {
      const accessToken = await getAccessToken(page);
      if (accessToken) {
        await cleanupWorkflowSmokeSessions(accessToken, options, log);
      }
    } catch {
      // Ignore cleanup failures.
    }
    await context.close();
    await browser.close();
  }
}

run().catch((error) => {
  console.error(`[test-embed-lifecycle-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
