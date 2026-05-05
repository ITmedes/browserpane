import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  DEFAULTS,
  cleanupWorkflowSmokeSessions,
  createLogger,
  fetchAuthConfig,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function ensureAdminLoggedIn(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  const authConfig = await fetchAuthConfig(options);
  if (!authConfig || authConfig.mode !== 'oidc') {
    return authConfig;
  }
  const loginButton = page.getByTestId('admin-login');
  if (!(await loginButton.isVisible().catch(() => false))) {
    await page.getByTestId('session-new').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    return authConfig;
  }
  if (!authConfig.exampleUser?.username || !authConfig.exampleUser?.password) {
    throw new Error('OIDC auth is enabled, but no example user is configured for smoke login.');
  }

  await loginButton.click();
  const username = page.locator('input[name="username"], #username').first();
  const password = page.locator('input[name="password"], #password').first();
  await username.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await password.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await username.fill(authConfig.exampleUser.username);
  await password.fill(authConfig.exampleUser.password);
  await page.locator('input[type="submit"], #kc-login').click();
  await page.waitForURL(urlPatternFor(options.pageUrl), { timeout: options.connectTimeoutMs });
  await page.getByTestId('session-new').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  return authConfig;
}

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-session-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  const log = createLogger('admin-session-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  let sessionId = '';

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    const accessToken = await getAdminAccessToken(page);
    if (accessToken) {
      await cleanupWorkflowSmokeSessions(accessToken, rootApiOptions(options), log);
    }

    log('Creating an admin-owned session.');
    await page.getByTestId('session-new').click();
    const row = page.getByTestId('session-row').first();
    await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    sessionId = await row.getAttribute('data-session-id') ?? '';
    if (!sessionId) {
      throw new Error('Admin session row did not expose a session id.');
    }

    log(`Connecting embedded browser for ${sessionId}.`);
    await page.getByTestId('browser-connect').click();
    const connectStatus = await poll(
      'admin embedded browser connection',
      async () => await page.getByTestId('browser-status').textContent(),
      (status) => status?.startsWith('Connected to') === true || status === 'Connection failed',
      options.connectTimeoutMs,
    );
    if (connectStatus === 'Connection failed') {
      const detail = await page.getByTestId('browser-error').textContent().catch(() => '');
      throw new Error(`Admin embedded browser connection failed${detail ? `: ${detail}` : ''}`);
    }

    const stopDisabled = await page.getByTestId('session-stop').isDisabled();
    if (!stopDisabled) {
      throw new Error('Expected session stop to be disabled while embedded browser is connected.');
    }

    const summary = { pageUrl: options.pageUrl, sessionId, stopDisabled };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    await cleanupAdminSession(page).catch(() => {});
    const accessToken = await getAdminAccessToken(page).catch(() => '');
    if (accessToken) {
      await cleanupWorkflowSmokeSessions(accessToken, rootApiOptions(options), log).catch(() => {});
    }
    await context.close();
    await browser.close();
  }
}

async function cleanupAdminSession(page) {
  if (await page.getByTestId('browser-disconnect').isEnabled().catch(() => false)) {
    await page.getByTestId('browser-disconnect').click();
  }
  if (await page.getByTestId('session-kill').isEnabled().catch(() => false)) {
    await page.getByTestId('session-kill').click();
  }
}

function urlPatternFor(pageUrl) {
  const url = new URL(pageUrl);
  const prefix = `${url.origin}${url.pathname}`;
  return new RegExp(`^${prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`);
}

async function getAdminAccessToken(page) {
  return await page.evaluate(() => {
    const raw = window.sessionStorage.getItem('bpane.admin.auth.tokens.v1');
    if (!raw) {
      return '';
    }
    const tokens = JSON.parse(raw);
    return typeof tokens.access_token === 'string' ? tokens.access_token : '';
  });
}

function rootApiOptions(options) {
  return {
    ...options,
    pageUrl: new URL('/', options.pageUrl).origin,
  };
}

run().catch((error) => {
  console.error(`[admin-session-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
