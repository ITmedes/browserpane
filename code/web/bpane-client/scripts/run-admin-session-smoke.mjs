import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  disconnectEmbeddedBrowser,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
  waitForBrowserConnected,
  waitForSessionState,
  waitForStopEnabled,
} from './admin-smoke-lib.mjs';
import { DEFAULTS, apiOrigin, createLogger, fetchJson, launchChrome, parseSmokeArgs, poll } from './workflow-smoke-lib.mjs';

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
    await verifyIdentityPanel(page, options);
    await expectSessionDisconnectControl(page);
    await verifySessionSwitchDisconnect(page, options, sessionId);
    await expectGlobalMessage(page, options, (message) => message.trim().length > 0);
    await dismissGlobalMessage(page, options);
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

    log('Disconnecting through the session inspector and releasing the selected session runtime.');
    await disconnectThroughSessionInspector(page, options);
    await waitForStopEnabled(page, options, sessionId);
    await page.getByTestId('session-release-runtime').click();
    await waitForSessionState(page, options, sessionId, 'released');
    await expectLifecycleMessage(page, options, 'Selected session runtime was released.');
    await expectRuntimeResumeMode(page, options, 'released');

    log(`Reconnecting released session ${sessionId}.`);
    await openAdminTab(page, 'sessions');
    await page.getByTestId('session-join').click();
    await waitForBrowserConnected(page, options);
    await openAdminTab(page, 'lifecycle');
    await expectRuntimeResumeMode(page, options, 'profile_restart');
    await disconnectThroughSessionArea(page, options);

    log(`Stopping reconnected session ${sessionId}.`);
    await waitForStopEnabled(page, options, sessionId);
    await page.getByTestId('session-stop').click();
    await waitForSessionState(page, options, sessionId, 'stopped');
    await expectLifecycleMessage(page, options, 'Selected session stopped.');
    await reconnectStoppedSession(page, options, sessionId);
    await disconnectEmbeddedBrowser(page, options);
    await waitForStopEnabled(page, options, sessionId);
    await page.getByTestId('session-stop').click();
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

async function verifyIdentityPanel(page, options) {
  const accessToken = await getAdminAccessToken(page);
  const identity = await fetchJson(`${apiOrigin(options)}/api/v1/identity/me`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const project = await createIdentityCrudProject(accessToken, options);

  await openAdminTab(page, 'identity');
  await page.getByTestId('identity-principal-type').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await poll(
    'admin identity refresh enabled',
    async () => await page.getByTestId('identity-refresh').isEnabled(),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('identity-refresh').click();
  await poll(
    'admin identity session count',
    async () => Number(await page.getByTestId('identity-resource-sessions').textContent()),
    (count) => Number.isFinite(count) && count >= 1,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('identity-project-list').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('identity-service-principal-list').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('identity-mapping-list').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await verifyServicePrincipalCrud(page, options, identity, project);
  await verifyIdentityMappingCrud(page, options, identity, project);
  const mappingCount = Number(await page.getByTestId('identity-resource-identity-mappings').textContent());
  if (!Number.isFinite(mappingCount) || mappingCount < 0) {
    throw new Error(`Expected identity mapping count to be numeric, got ${mappingCount}`);
  }
  await page.getByTestId('identity-delegation-list').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

async function createIdentityCrudProject(accessToken, options) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: `admin-identity-crud-${Date.now()}`,
      labels: { suite: 'admin-session-identity-crud-smoke' },
    }),
  });
}

async function verifyServicePrincipalCrud(page, options, identity, project) {
  const suffix = Date.now();
  const principalName = `Admin service principal ${suffix}`;
  const updatedName = `${principalName} updated`;
  const clientId = `admin-sp-${suffix}`;

  await page.getByTestId('identity-service-principal-new').click();
  await page.getByTestId('identity-service-principal-name').fill(principalName);
  await page.getByTestId('identity-service-principal-client-id').fill(clientId);
  await page.getByTestId('identity-service-principal-issuer').fill(identity.issuer);
  await page.getByTestId('identity-service-principal-projects').selectOption(project.id);
  await page.getByTestId('identity-service-principal-scopes').fill('session:create\nsession:delegate');
  await page.getByTestId('identity-service-principal-labels').fill('suite=admin-session-identity-crud-smoke');
  await page.getByTestId('identity-service-principal-save').click();
  await poll(
    'admin service principal created',
    async () => await page.getByTestId('identity-service-principal-selected-name').textContent().catch(() => ''),
    (value) => value === principalName,
    options.connectTimeoutMs,
    100,
  );
  await poll(
    'admin service principal project name rendered',
    async () => await page.getByTestId('identity-service-principal-selected-projects').textContent().catch(() => ''),
    (value) => value.includes(project.name),
    options.connectTimeoutMs,
    100,
  );

  await page.getByTestId('identity-service-principal-edit').click();
  await page.getByTestId('identity-service-principal-name').fill(updatedName);
  await page.getByTestId('identity-service-principal-save').click();
  await poll(
    'admin service principal updated',
    async () => await page.getByTestId('identity-service-principal-selected-name').textContent().catch(() => ''),
    (value) => value === updatedName,
    options.connectTimeoutMs,
    100,
  );

  await page.getByTestId('identity-service-principal-disable').click();
  await poll(
    'admin service principal disabled',
    async () => await page.getByTestId('identity-service-principal-enable').isEnabled().catch(() => false),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('identity-service-principal-enable').click();
  await poll(
    'admin service principal re-enabled',
    async () => await page.getByTestId('identity-service-principal-disable').isEnabled().catch(() => false),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
}

async function verifyIdentityMappingCrud(page, options, identity, project) {
  const mappingName = `Admin identity CRUD ${Date.now()}`;
  const updatedName = `${mappingName} updated`;

  await page.getByTestId('identity-mapping-new').click();
  await page.getByTestId('identity-mapping-name').fill(mappingName);
  await page.getByTestId('identity-mapping-kind').selectOption('user');
  await page.getByTestId('identity-mapping-issuer').fill(identity.issuer);
  await page.getByTestId('identity-mapping-external-id').fill(identity.subject);
  await page.getByTestId('identity-mapping-project-id').selectOption(project.id);
  await page.getByTestId('identity-mapping-scopes').fill('session:create\nsession:delegate');
  await page.getByTestId('identity-mapping-labels').fill('suite=admin-session-identity-crud-smoke');
  await page.getByTestId('identity-mapping-save').click();
  await poll(
    'admin identity mapping created',
    async () => await page.getByTestId('identity-mapping-selected-name').textContent().catch(() => ''),
    (value) => value === mappingName,
    options.connectTimeoutMs,
    100,
  );
  await poll(
    'admin identity mapping project name rendered',
    async () => await page.getByTestId('identity-mapping-selected-project-id').textContent().catch(() => ''),
    (value) => value.includes(project.name),
    options.connectTimeoutMs,
    100,
  );

  await page.getByTestId('identity-mapping-edit').click();
  await page.getByTestId('identity-mapping-name').fill(updatedName);
  await page.getByTestId('identity-mapping-save').click();
  await poll(
    'admin identity mapping updated',
    async () => await page.getByTestId('identity-mapping-selected-name').textContent().catch(() => ''),
    (value) => value === updatedName,
    options.connectTimeoutMs,
    100,
  );

  await page.getByTestId('identity-mapping-disable').click();
  await poll(
    'admin identity mapping disabled',
    async () => await page.getByTestId('identity-mapping-enable').isEnabled().catch(() => false),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('identity-mapping-enable').click();
  await poll(
    'admin identity mapping re-enabled',
    async () => await page.getByTestId('identity-mapping-disable').isEnabled().catch(() => false),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
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
  await expectLifecycleMessage(page, options, 'Disconnected all live clients.');
}

async function disconnectThroughSessionArea(page, options) {
  await openAdminTab(page, 'sessions');
  await page.getByTestId('session-disconnect').click();
  await poll(
    'admin embedded browser disconnect through session area',
    async () => await page.getByTestId('browser-status').textContent(),
    (status) => status === 'Disconnected',
    options.connectTimeoutMs,
  );
}

async function expectSessionDisconnectControl(page) {
  await openAdminTab(page, 'sessions');
  const disconnectEnabled = await page.getByTestId('session-disconnect').isEnabled();
  if (!disconnectEnabled) {
    throw new Error('Expected session area disconnect control to be enabled after browser connect.');
  }
}

async function verifySessionSwitchDisconnect(page, options, originalSessionId) {
  const accessToken = await getAdminAccessToken(page);
  const switchTarget = await createSwitchTargetSession(accessToken, options);

  await openAdminTab(page, 'sessions');
  await waitForSessionRow(page, options, switchTarget.id);
  await page.locator(`[data-testid="session-row"][data-session-id="${switchTarget.id}"]`).click();
  await poll(
    'admin browser disconnect on session switch',
    async () => await page.getByTestId('browser-status').textContent(),
    (status) => status === 'Disconnected',
    options.connectTimeoutMs,
    100,
  );
  const disconnectEnabled = await page.getByTestId('session-disconnect').isEnabled();
  if (disconnectEnabled) {
    throw new Error('Expected session area disconnect control to be disabled after switching away from the live session.');
  }
  await stopSwitchTargetSession(accessToken, options, switchTarget.id);

  await page.locator(`[data-testid="session-row"][data-session-id="${originalSessionId}"]`).click();
  await poll(
    'admin session join enabled after switch-back',
    async () => await page.getByTestId('session-join').isEnabled(),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('session-join').click();
  await waitForBrowserConnected(page, options);
  await expectSessionDisconnectControl(page);
}

async function createSwitchTargetSession(accessToken, options) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ labels: { suite: 'admin-session-switch-smoke' } }),
  });
}

async function stopSwitchTargetSession(accessToken, options, sessionId) {
  await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/kill`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function waitForSessionRow(page, options, sessionId) {
  await poll(
    `admin session row ${sessionId}`,
    async () => {
      const row = page.locator(`[data-testid="session-row"][data-session-id="${sessionId}"]`);
      return await row.isVisible().catch(() => false);
    },
    Boolean,
    options.connectTimeoutMs,
    100,
  );
}

async function expectLifecycleMessage(page, options, expectedText) {
  await poll(
    `admin lifecycle message ${expectedText}`,
    async () => await page.getByTestId('session-lifecycle-message').textContent().catch(() => ''),
    (message) => message.includes(expectedText),
    options.connectTimeoutMs,
    100,
  );
}

async function expectRuntimeResumeMode(page, options, expectedText) {
  await openAdminTab(page, 'lifecycle');
  await page.getByTestId('session-detail-refresh').click();
  await poll(
    `admin runtime resume mode ${expectedText}`,
    async () => await page.getByTestId('session-runtime-resume-mode').textContent().catch(() => ''),
    (value) => value === expectedText,
    options.connectTimeoutMs,
    100,
  );
}

async function reconnectStoppedSession(page, options, sessionId) {
  await openAdminTab(page, 'sessions');
  await poll(
    'admin stopped session join enabled',
    async () => await page.getByTestId('session-join').isEnabled(),
    Boolean,
    options.connectTimeoutMs,
    100,
  );
  await page.getByTestId('session-join').click();
  await waitForBrowserConnected(page, options);
  await expectRuntimeResumeMode(page, options, 'profile_restart');
}

async function expectGlobalMessage(page, options, matches) {
  await page.getByTestId('admin-global-message-region').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await poll(
    'admin global message',
    async () => await page.getByTestId('admin-global-message').textContent().catch(() => ''),
    matches,
    options.connectTimeoutMs,
    100,
  );
}

async function dismissGlobalMessage(page, options) {
  await page.getByTestId('admin-global-message-dismiss').click();
  await page.getByTestId('admin-global-message-region').waitFor({ state: 'hidden', timeout: options.connectTimeoutMs });
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
