import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn, getAdminAccessToken } from './admin-smoke-lib.mjs';
import { exerciseCredentialCatalog } from './admin-unified-credential-catalog-smoke.mjs';
import { exerciseEventCatalog } from './admin-unified-event-catalog-smoke.mjs';
import { exerciseExtensionCatalog } from './admin-unified-extension-catalog-smoke.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs } from './workflow-smoke-lib.mjs';

const projectRoot = path.resolve(import.meta.dirname, '../../../..');

async function run() {
  const options = parseSmokeArgs(
    process.argv.slice(2),
    'run-admin-unified-resource-catalogs-smoke.mjs',
  );
  if (options.pageUrl === DEFAULTS.pageUrl) options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/`;
  const log = createLogger('admin-unified-resource-catalogs-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const runLabel = `resource-catalogs-${Date.now()}`;
  const credentialSecret = `credential-${runLabel}`;
  const signingSecret = `signing-${runLabel}`;
  const browserLogs = [];
  page.on('console', (message) => browserLogs.push(message.text()));

  try {
    log(`Authenticating through ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    const accessToken = await getAdminAccessToken(page);
    assert.ok(accessToken, 'admin resource catalog smoke requires an owner access token');
    const extension = await exerciseExtensionCatalog(page, options, runLabel);
    const credential = await exerciseCredentialCatalog(page, options, runLabel, credentialSecret);
    const eventSubscription = await exerciseEventCatalog(
      page,
      options,
      accessToken,
      runLabel,
      signingSecret,
    );
    assertSecretsExcluded(
      browserLogs.join('\n'),
      [credentialSecret, signingSecret],
      'browser logs',
    );
    assertSecretsExcluded(gatewayLogs(), [credentialSecret, signingSecret], 'gateway logs');
    console.log(
      JSON.stringify({ extension, credential, eventSubscription, secretsRedacted: true }, null, 2),
    );
  } finally {
    await context.close();
    await browser.close();
  }
}

function gatewayLogs() {
  return execFileSync(
    'docker',
    [
      'compose',
      '-f',
      path.join(projectRoot, 'deploy/compose.yml'),
      'logs',
      '--no-color',
      'gateway',
    ],
    { cwd: projectRoot, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  );
}

function assertSecretsExcluded(text, secrets, label) {
  for (const secret of secrets) {
    assert.equal(text.includes(secret), false, `${label} contained a write-only secret`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-resource-catalogs-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
