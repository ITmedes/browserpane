import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';

import { ensureAdminLoggedIn } from './admin-smoke-lib.mjs';
import {
  createLogger,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(
    process.argv.slice(2),
    'run-admin-unified-promotion-smoke.mjs',
  );
  const origin = new URL('/', options.pageUrl).origin;
  const rootUrl = `${origin}/`;
  const log = createLogger('admin-unified-promotion-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();

  try {
    log(`Checking promoted root ${rootUrl}`);
    await assertRootRedirect(rootUrl);
    await ensureAdminLoggedIn(page, { ...options, pageUrl: rootUrl });
    await assertPath(page, '/admin-new/');
    await page.getByTestId('dashboard-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    const sessionsUrl = `${origin}/admin-new/sessions`;
    log(`Checking refresh-safe deep link ${sessionsUrl}`);
    await page.goto(sessionsUrl, { waitUntil: 'domcontentloaded' });
    await ensureAdminLoggedIn(page, { ...options, pageUrl: sessionsUrl });
    await page.getByTestId('sessions-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.getByTestId('sessions-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertPath(page, '/admin-new/sessions');

    const fallbackUrl = `${origin}/admin/`;
    log(`Checking compatibility fallback ${fallbackUrl}`);
    await ensureAdminLoggedIn(page, { ...options, pageUrl: fallbackUrl });
    await assertPath(page, '/admin/');
    await page.getByTestId('admin-overlay').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    await ensureAdminLoggedIn(page, { ...options, pageUrl: rootUrl });
    await assertPath(page, '/admin-new/');
    await page.getByTestId('dashboard-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });

    const summary = {
      rootRedirect: '/admin-new/',
      unifiedDeepLink: '/admin-new/sessions',
      compatibilityFallback: '/admin/',
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
  } finally {
    await context.close();
    await browser.close();
  }
}

async function assertRootRedirect(rootUrl) {
  const response = await fetch(rootUrl, { redirect: 'manual' });
  const location = response.headers.get('location');

  assert.equal(response.status, 302);
  assert.ok(location, 'root redirect must include a Location header');
  const redirect = new URL(location, rootUrl);
  assert.equal(redirect.origin, new URL(rootUrl).origin);
  assert.equal(redirect.pathname, '/admin-new/');
  assert.equal(redirect.search, '');
  assert.equal(redirect.hash, '');
  assert.equal(response.headers.get('cache-control'), 'no-cache, no-store, must-revalidate');
}

async function assertPath(page, expectedPath) {
  const current = new URL(page.url());
  assert.equal(current.pathname, expectedPath);
  assert.equal(current.search, '', `unexpected query parameters on ${expectedPath}`);
  assert.equal(current.hash, '', `unexpected URL fragment on ${expectedPath}`);
}

run().catch((error) => {
  console.error(`[admin-unified-promotion-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
