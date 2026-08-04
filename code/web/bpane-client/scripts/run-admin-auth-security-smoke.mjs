import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn, getAdminAccessToken } from './admin-smoke-lib.mjs';
import {
  createLogger,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

const TOKEN_KEY = 'bpane.admin.auth.tokens.v2';
const LEGACY_TOKEN_KEY = 'bpane.admin.auth.tokens.v1';
const ROUTES = [
  { path: '/admin/', logoutTestId: 'admin-logout' },
  { path: '/admin-new/', logoutTestId: 'admin-new-account' },
];

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-auth-security-smoke.mjs');
  const origin = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-auth-security-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const summaries = [];

  try {
    for (const route of ROUTES) {
      const pageUrl = new URL(route.path, origin).toString();
      const routeOptions = { ...options, pageUrl };
      const page = await context.newPage();
      const policyErrors = [];
      const browserErrors = [];
      page.on('console', (message) => {
        const text = message.text();
        if (text.includes('Content Security Policy') || text.startsWith('Refused to ')) {
          policyErrors.push(text);
        }
        if (message.type() === 'error') {
          browserErrors.push(text);
        }
      });
      page.on('pageerror', (error) => browserErrors.push(error.message));

      try {
        log(`Checking headers and authentication recovery at ${pageUrl}`);
        await assertSecurityHeaders(pageUrl);
        await ensureAdminLoggedIn(page, routeOptions);
        await assertDocumentCsp(page);
        await assertMemoryOnlyTokens(page);
        assert.ok(await getAdminAccessToken(page), 'expected in-memory bearer use after login');

        await page.reload({ waitUntil: 'domcontentloaded' });
        await ensureAdminLoggedIn(page, routeOptions);
        await assertMemoryOnlyTokens(page);
        assert.ok(await getAdminAccessToken(page), 'expected in-memory bearer use after SSO reload');

        await page.getByTestId(route.logoutTestId).click();
        await ensureAdminLoggedIn(page, routeOptions);
        await assertMemoryOnlyTokens(page);
        assert.deepEqual(policyErrors, [], `CSP violations at ${route.path}`);
        summaries.push({ route: route.path, authenticated: true, recovery: true, headers: true });
      } catch (error) {
        console.error(JSON.stringify(await redactedPageDiagnostics(page, browserErrors), null, 2));
        throw error;
      } finally {
        await page.close();
      }
    }

    console.log(JSON.stringify({ routes: summaries }, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify({ routes: summaries }, null, 2)}\n`, 'utf8');
    }
  } finally {
    await context.close();
    await browser.close();
  }
}

async function assertSecurityHeaders(url) {
  const response = await fetch(url, { redirect: 'follow' });
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('x-content-type-options'), 'nosniff');
  assert.equal(response.headers.get('x-frame-options'), 'DENY');
  assert.equal(response.headers.get('referrer-policy'), 'no-referrer');
  assert.match(response.headers.get('permissions-policy') ?? '', /camera=\(self\)/);
  const policy = response.headers.get('content-security-policy') ?? '';
  for (const directive of ["frame-ancestors 'none'", "object-src 'none'"]) {
    assert.ok(policy.includes(directive), `missing CSP directive ${directive}`);
  }
}

async function assertDocumentCsp(page) {
  const policy = await page.locator('meta[http-equiv="content-security-policy"]').getAttribute('content');
  assert.ok(policy?.includes("default-src 'self'"));
  assert.ok(policy?.includes("script-src 'self' 'sha256-"));
  assert.ok(policy?.includes("object-src 'none'"));
}

async function assertMemoryOnlyTokens(page) {
  const state = await page.evaluate(({ tokenKey, legacyTokenKey }) => ({
    current: window.sessionStorage.getItem(tokenKey),
    legacy: window.sessionStorage.getItem(legacyTokenKey),
    tokenKeys: Object.keys(window.sessionStorage).filter((key) => key.includes('auth.tokens')),
    tokenValues: Object.values(window.sessionStorage).filter((value) => (
      value.includes('access-token')
      || value.includes('refresh-token')
      || value.includes('id_token')
      || value.includes('access_token')
      || value.includes('refresh_token')
    )),
  }), { tokenKey: TOKEN_KEY, legacyTokenKey: LEGACY_TOKEN_KEY });
  assert.equal(state.current, null);
  assert.equal(state.legacy, null);
  assert.deepEqual(state.tokenKeys, []);
  assert.deepEqual(state.tokenValues, []);
}

async function redactedPageDiagnostics(page, browserErrors) {
  return await page.evaluate(() => ({
    url: `${window.location.origin}${window.location.pathname}`,
    visibleText: document.body.innerText.slice(0, 1_000),
    sessionStorageKeys: Object.keys(window.sessionStorage),
  })).then((state) => ({ ...state, browserErrors })).catch(() => ({ browserErrors }));
}

run().catch((error) => {
  console.error(`[admin-auth-security-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
