import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  disconnectEmbeddedBrowser,
  ensureAdminLoggedIn,
  openAdminTab,
  waitForBrowserConnected,
  waitForKillEnabled,
  waitForSessionState,
  waitForStopEnabled,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, createLogger, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

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
    await cleanupAdminBeforeRun(page, options, log);

    await openAdminTab(page, 'display');
    await configureDisplayControls(page);

    log('Creating and joining an admin-owned session.');
    await openAdminTab(page, 'sessions');
    await page.getByTestId('session-new').click();
    sessionId = await resolveSelectedSessionId(page, options);
    await waitForMcpDelegationReady(page, options);
    await waitForBrowserConnected(page, options);
    await verifyBrowserPolicyPanel(page);
    await verifyRemainingPanels(page);
    await openAdminTab(page, 'display');
    const uploadEnabled = await page.getByTestId('display-upload').isEnabled();
    if (!uploadEnabled) {
      throw new Error('Expected display upload control to be enabled after browser connect.');
    }
    await verifyDisplayUpload(page, options);
    await openAdminTab(page, 'lifecycle');
    await verifySessionStatusInspector(page, options);
    const stopDisabled = await page.getByTestId('session-stop').isDisabled();
    if (!stopDisabled) {
      throw new Error('Expected session stop to be disabled while embedded browser is connected.');
    }

    log('Disconnecting through the session inspector and stopping the selected session.');
    await disconnectThroughSessionInspector(page, options);
    await waitForStopEnabled(page, options, sessionId);
    await page.getByTestId('session-stop').click();
    await waitForSessionState(page, options, sessionId, 'stopped');

    log(`Reconnecting stopped session ${sessionId}.`);
    await openAdminTab(page, 'sessions');
    await page.getByTestId('session-join').click();
    await waitForBrowserConnected(page, options);
    await disconnectEmbeddedBrowser(page, options);

    log(`Force killing reconnected session ${sessionId}.`);
    await waitForKillEnabled(page, options, sessionId);
    await page.getByTestId('session-kill').click();
    await waitForSessionState(page, options, sessionId, 'stopped');
    await emitSummary(page, options, sessionId, stopDisabled, log);
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

async function verifyBrowserPolicyPanel(page) {
  await openAdminTab(page, 'policy');
  const policyMode = await page.getByTestId('policy-mode').textContent();
  if (!policyMode?.includes('deny_all')) {
    throw new Error(`Expected admin policy panel to report deny_all, got ${policyMode}`);
  }
  const fileUrlPolicy = await page.getByTestId('policy-file-url').textContent();
  if (fileUrlPolicy !== 'blocked') {
    throw new Error(`Expected file URL policy to be blocked, got ${fileUrlPolicy}`);
  }
  const copyEnabled = await page.getByTestId('policy-copy-command').isEnabled();
  if (!copyEnabled) {
    throw new Error('Expected policy CDP probe command to be copyable after browser connect.');
  }
}

async function verifyRemainingPanels(page) {
  await openAdminTab(page, 'recording');
  await page.getByTestId('recording-status').waitFor({ state: 'visible' });
  await openAdminTab(page, 'metrics');
  await page.getByTestId('metrics-sample').waitFor({ state: 'visible' });
  await openAdminTab(page, 'logs');
  await page.getByTestId('admin-log-count').waitFor({ state: 'visible' });
  await openAdminTab(page, 'workflows');
  await page.getByTestId('workflow-status').waitFor({ state: 'visible' });
}

async function verifySessionStatusInspector(page, options) {
  await page.getByTestId('session-total-clients').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('session-connection-row').first().waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const disconnectAllEnabled = await page.getByTestId('session-disconnect-all').isEnabled();
  if (!disconnectAllEnabled) {
    throw new Error('Expected session inspector disconnect-all to be enabled with a live connection.');
  }
}

async function disconnectThroughSessionInspector(page, options) {
  await page.getByTestId('session-disconnect-all').click();
  await poll(
    'admin embedded browser disconnect through session inspector',
    async () => await page.getByTestId('browser-status').textContent(),
    (status) => status === 'Disconnected',
    options.connectTimeoutMs,
  );
}

async function waitForMcpDelegationReady(page, options) {
  await openAdminTab(page, 'sessions');
  await page.getByTestId('mcp-status').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await poll('admin MCP delegate button enabled', async () => {
    return await page.getByTestId('mcp-delegate').isEnabled();
  }, (enabled) => enabled, options.connectTimeoutMs);
}

async function configureDisplayControls(page) {
  await openAdminTab(page, 'display');
  await page.getByTestId('display-render-backend').selectOption('canvas2d');
  await page.getByTestId('display-hidpi').setChecked(false);
  await page.getByTestId('display-scroll-copy').setChecked(false);
  const uploadDisabled = await page.getByTestId('display-upload').isDisabled();
  if (!uploadDisabled) {
    throw new Error('Expected display upload control to stay disabled before browser connect.');
  }
}

async function verifyDisplayUpload(page, options) {
  await openAdminTab(page, 'display');
  await page.getByTestId('display-upload-input').setInputFiles({
    name: 'admin-upload-smoke.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('BrowserPane admin upload smoke\n'),
  });
  await page.waitForTimeout(250);
  const state = await poll('admin display upload completion', async () => ({
    busy: await page.getByTestId('display-busy').isVisible().catch(() => false),
    error: await page.getByTestId('display-error').textContent().catch(() => ''),
  }), (value) => Boolean(value.error) || !value.busy, options.connectTimeoutMs, 100);
  if (state.error) {
    throw new Error(`Admin display upload failed: ${state.error}`);
  }
}

async function resolveSelectedSessionId(page, options) {
  const row = page.getByTestId('session-row').first();
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const sessionId = await row.getAttribute('data-session-id') ?? '';
  if (!sessionId) {
    throw new Error('Admin session row did not expose a session id.');
  }
  return sessionId;
}

async function emitSummary(page, options, sessionId, stopDisabled, log) {
  await openAdminTab(page, 'lifecycle');
  const summary = {
    pageUrl: options.pageUrl,
    sessionId,
    stopDisabledWhileConnected: stopDisabled,
    finalState: await page.getByTestId('session-state').textContent(),
  };
  console.log(JSON.stringify(summary, null, 2));
  if (options.outputPath) {
    await fs.writeFile(options.outputPath, JSON.stringify(summary, null, 2));
    log(`Wrote summary to ${options.outputPath}`);
  }
}

run().catch((error) => {
  console.error(`[admin-session-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
