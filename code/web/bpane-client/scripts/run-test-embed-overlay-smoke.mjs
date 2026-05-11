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

function buildBrowserOnlyPageUrl(pageUrl) {
  const url = new URL(testEmbedPageUrl({ pageUrl }));
  url.searchParams.set('layout', 'browser-only');
  return url.toString();
}

async function configureEmbedPage(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
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

  const overlayLogin = page.locator('#overlay-login-btn');
  const sidebarLogin = page.locator('#btn-login');
  const loginControls = await poll(
    'visible login action',
    async () => ({
      overlayVisible: await overlayLogin.isVisible().catch(() => false),
      sidebarVisible: await sidebarLogin.isVisible().catch(() => false),
    }),
    (value) => value.overlayVisible || value.sidebarVisible,
    options.connectTimeoutMs,
  );
  if (loginControls.overlayVisible) {
    await overlayLogin.click();
  } else {
    await sidebarLogin.click();
  }

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

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-test-embed-overlay-smoke.mjs');
  const log = createLogger('test-embed-overlay-smoke');
  const pageUrl = buildBrowserOnlyPageUrl(options.pageUrl);

  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();

    let createdSessionId = '';
  try {
    log(`Opening ${pageUrl}`);
    await configureEmbedPage(page, { ...options, pageUrl });
    await ensureEmbedLoggedIn(page, { ...options, pageUrl });

    const accessToken = await getAccessToken(page);
    if (accessToken) {
      await cleanupWorkflowSmokeSessions(accessToken, options, log);
      await page.evaluate(async () => {
        await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
      });
    }

    const postLoginState = await poll(
      'post-login browser-only overlay state',
      async () => await page.evaluate(() => ({
        href: window.location.href,
        browserOnlyLayout: document.body.classList.contains('browser-only'),
        loginVisible: !document.getElementById('overlay-login-btn')?.hidden,
        startVisible: !document.getElementById('overlay-start-btn')?.hidden,
        startEnabled: !(document.getElementById('overlay-start-btn')?.disabled ?? true),
        overlayVisible: getComputedStyle(document.getElementById('overlay')).display !== 'none',
      })),
      (value) => value.browserOnlyLayout && value.startVisible && value.startEnabled && value.overlayVisible,
      options.connectTimeoutMs,
    );

    const postLoginUrl = new URL(postLoginState.href);
    if (postLoginUrl.searchParams.get('layout') !== 'browser-only') {
      throw new Error(`Expected layout=browser-only after login, got ${postLoginUrl.search}`);
    }
    if (postLoginState.loginVisible) {
      throw new Error('Overlay still shows the login action after authentication.');
    }

    log('Clicking the overlay start action.');
    await page.click('#overlay-start-btn');

    const connectedState = await poll(
      'browser-only session connection',
      async () => await page.evaluate(() => ({
        overlayVisible: getComputedStyle(document.getElementById('overlay')).display !== 'none',
        control: window.__bpaneControl.getState(),
      })),
      (value) => value.control?.connected === true && value.overlayVisible === false,
      options.connectTimeoutMs,
    );
    createdSessionId = connectedState.control?.sessionId ?? '';
    if (!createdSessionId) {
      throw new Error('Expected a connected session id after starting from the overlay.');
    }

    const summary = {
      pageUrl,
      sessionId: createdSessionId,
      layout: postLoginUrl.searchParams.get('layout'),
      connected: connectedState.control.connected,
      status: connectedState.control.status,
    };
    log(`Connected session ${createdSessionId}`);
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
      // Ignore page cleanup failures.
    }
    try {
      const accessToken = await getAccessToken(page);
      if (accessToken) {
        await cleanupWorkflowSmokeSessions(accessToken, options, log);
      }
    } catch {
      // Ignore cleanup failures here so the smoke reports the real assertion error.
    }
    await context.close();
    await browser.close();
  }
}

run().catch((error) => {
  console.error(`[test-embed-overlay-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
