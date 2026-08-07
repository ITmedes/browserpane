import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  findActiveSessionIdsByLabels,
  waitForContains,
} from './admin-unified-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  fetchAuthConfig,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-sessions-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/sessions`;
  }
  const log = createLogger('admin-unified-sessions-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  await context.grantPermissions(['clipboard-write'], { origin: apiOrigin(options) }).catch(() => {});
  const page = await context.newPage();
  let accessToken = '';
  let createdSessionId = '';
  let authConfig = null;

  try {
    log(`Opening ${options.pageUrl}`);
    authConfig = await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }
    await cleanupStaleSessionSmokes(accessToken, options, log);

    await page.goto(adminRouteUrl(options, 'sessions'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('sessions-overview').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified sessions catalog');
    await assertNoHorizontalOverflow(page, 'sessions-overview', 'unified sessions overview');

    const newSessionHref = await page.getByTestId('sessions-new').getAttribute('href');
    if (!newSessionHref?.endsWith('/admin-new/sessions/new')) {
      throw new Error(`Expected New session to open the create form, got ${newSessionHref}`);
    }
    await page.getByTestId('sessions-new').click();
    await page.getByTestId('session-create-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified session create');
    await assertNoHorizontalOverflow(page, 'session-create-route', 'unified session create route');
    await page.getByTestId('session-create-capability-camera').uncheck();
    await page.getByTestId('session-create-capability-microphone').uncheck();
    await page.getByTestId('session-create-labels').fill('suite=admin-unified-sessions\npurpose=smoke');
    const preview = await page.getByTestId('session-create-payload').textContent();
    if (preview?.includes('bpane_admin_surface')) {
      throw new Error(`Session create form added an implicit admin label: ${preview}`);
    }
    if (!preview?.includes('"camera": false') || !preview.includes('"microphone": false')) {
      throw new Error(`Session create form did not include capability overrides: ${preview}`);
    }
    const createResponsePromise = page.waitForResponse((response) => {
      const request = response.request();
      const url = new URL(response.url());
      return request.method() === 'POST' && url.pathname === '/api/v1/sessions';
    }, { timeout: options.connectTimeoutMs });
    await page.getByTestId('session-create-save').click();
    const createResponse = await createResponsePromise;
    if (createResponse.ok()) {
      const createdSession = await createResponse.json();
      createdSessionId = typeof createdSession?.id === 'string' ? createdSession.id : '';
    }
    createdSessionId = await waitForSessionDetailUrl(page, options, createdSessionId);
    await page.getByTestId('session-detail-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'session-detail-title', shortSessionId(createdSessionId));
    await page.getByTestId('session-detail-lifecycle').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.getByTestId('session-detail-runtime-section').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.getByTestId('session-detail-capabilities').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await waitForContains(page, options, 'session-capability-camera', 'disabled');
    await waitForContains(page, options, 'session-capability-microphone', 'disabled');
    await assertNoBodyHorizontalOverflow(page, 'unified session detail');
    await assertNoHorizontalOverflow(page, 'session-detail-route', 'unified session detail route');
    await assertNoHorizontalOverflow(page, 'session-inspector', 'unified session inspector');

    await page.getByTestId('session-detail-refresh').click();
    await waitForContains(page, options, 'session-detail-action-success', 'refreshed');
    await verifyMcpDelegation(page, options, createdSessionId, authConfig, accessToken);
    await verifySessionPreviewPopup(page, options);
    await verifyStoppedSessionCanStartWithPreview(page, options);

    console.log(JSON.stringify({
      sessionId: createdSessionId,
      detailVisible: true,
      mcpDelegation: true,
      previewPopup: true,
      stoppedSessionRestarted: true,
    }, null, 2));
  } finally {
    if (accessToken && createdSessionId) {
      await cleanupMcpDelegation(accessToken, options, createdSessionId, authConfig).catch((error) => {
        log(`MCP cleanup for ${createdSessionId} failed: ${error.message}`);
      });
      await cleanupSession(accessToken, options, createdSessionId).catch((error) => {
        log(`Session cleanup for ${createdSessionId} failed: ${error.message}`);
      });
    }
    await context.close();
    await browser.close();
  }
}

async function verifyMcpDelegation(page, options, sessionId, authConfig, accessToken) {
  const bridge = authConfig?.mcpBridge;
  if (!bridge?.controlUrl || !bridge.clientId) {
    throw new Error('Unified session smoke requires auth-config mcpBridge metadata.');
  }
  await page.getByTestId('session-mcp-delegation').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await waitForContains(page, options, 'mcp-delegation-status', 'Not authorized');
  await waitForContains(page, options, 'mcp-endpoint-url', `/sessions/${sessionId}/mcp`);

  await page.getByTestId('mcp-authorize').click();
  await waitForContains(page, options, 'mcp-action-success', 'authorized');
  await waitForContains(page, options, 'mcp-delegation-status', 'Authorized');

  await page.getByTestId('mcp-set-default').click();
  await waitForContains(page, options, 'mcp-delegation-status', 'Authorized default');
  await waitForContains(page, options, 'mcp-default-session', 'This session');

  await page.getByTestId('mcp-copy-endpoint').click();
  await waitForContains(page, options, 'mcp-action-success', 'copied');

  await page.getByTestId('mcp-clear-default').click();
  await waitForContains(page, options, 'mcp-default-session', 'No default');

  await page.getByTestId('mcp-revoke').click();
  await waitForContains(page, options, 'mcp-delegation-status', 'Not authorized');
  await assertMcpDelegationClean(accessToken, options, sessionId, bridge);
}

async function assertMcpDelegationClean(accessToken, options, sessionId, bridge) {
  const session = await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(sessionId)}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (session.automation_delegate !== null) {
    throw new Error(`Expected MCP automation delegate to be cleared, got ${JSON.stringify(session.automation_delegate)}`);
  }
  const health = await fetchJson(mcpHealthUrl(bridge));
  if (health.control_session_id === sessionId) {
    throw new Error(`Expected MCP default session to be cleared, still points to ${sessionId}`);
  }
}

async function cleanupMcpDelegation(accessToken, options, sessionId, authConfig) {
  const bridge = authConfig?.mcpBridge ?? (await fetchAuthConfig(options))?.mcpBridge;
  if (!bridge?.controlUrl) {
    return;
  }
  const health = await fetchJson(mcpHealthUrl(bridge)).catch(() => null);
  if (health?.control_session_id === sessionId) {
    await fetch(bridge.controlUrl, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${accessToken}` },
    });
  }
  await fetch(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  }).catch(() => {});
}

function mcpHealthUrl(bridge) {
  const healthUrl = new URL(bridge.endpointBaseUrl ?? bridge.controlUrl);
  healthUrl.pathname = '/health';
  healthUrl.search = '';
  healthUrl.hash = '';
  return healthUrl.toString();
}

async function verifySessionPreviewPopup(page, options, expectedActionLabel = 'Connect') {
  await waitForConnectPreviewEnabled(page, options, expectedActionLabel);
  const popupPromise = page.waitForEvent('popup');
  await page.getByTestId('session-connect-preview').click();
  const popup = await popupPromise;
  try {
    await popup.getByTestId('session-preview-route').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(popup, 'unified session preview popup');
    await assertNoHorizontalOverflow(popup, 'session-preview-route', 'unified session preview route');
    await waitForPreviewConnected(popup, options);
    await verifyPreviewMetrics(popup, options);
    await assertPreviewResizeUsesIndependentHeight(popup, options);
    await assertNoHorizontalOverflow(popup, 'session-preview-viewport', 'unified session preview viewport');
  } finally {
    await popup.close({ runBeforeUnload: true }).catch(() => {});
  }
}

async function verifyPreviewMetrics(popup, options) {
  await popup.getByTestId('session-preview-metrics-toggle').click();
  await popup.getByTestId('session-preview-metrics-drawer').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await poll(
    'session preview metrics start enabled',
    async () => await popup.getByTestId('session-preview-metrics-start').isEnabled().catch(() => false),
    Boolean,
    options.connectTimeoutMs,
  );
  await popup.getByTestId('session-preview-metrics-start').click();
  await waitForContains(popup, options, 'session-preview-metrics-sample', 'fps');
  await waitForContains(popup, options, 'session-preview-metrics-transfer', 'down');
  await waitForContains(popup, options, 'session-preview-metrics-tiles', 'commands');
  await waitForContains(popup, options, 'session-preview-metrics-scroll', 'saved');
  await waitForContains(popup, options, 'session-preview-metrics-video', 'datagrams');
  await popup.getByTestId('session-preview-metrics-copy').click();
  await waitForContains(popup, options, 'session-preview-metrics-message', 'copied');
  await poll(
    'session preview metrics stop enabled',
    async () => await popup.getByTestId('session-preview-metrics-stop').isEnabled().catch(() => false),
    Boolean,
    options.connectTimeoutMs,
  );
  await popup.getByTestId('session-preview-metrics-stop').click();
}

async function verifyStoppedSessionCanStartWithPreview(page, options) {
  await waitForStopActionEnabled(page, options);
  await page.getByTestId('session-stop').click();
  await waitForContains(page, options, 'session-detail-action-success', 'stopped');
  await waitForContains(page, options, 'session-detail-state', 'stopped');
  await verifySessionPreviewPopup(page, options, 'Start and connect');
}

async function waitForConnectPreviewEnabled(page, options, expectedActionLabel) {
  await poll(
    'session preview connect action',
    async () => {
      const action = page.getByTestId('session-connect-preview');
      const enabled = await action.isEnabled().catch(() => false);
      const text = await action.textContent().catch(() => '');
      if (enabled && text?.trim() === expectedActionLabel) {
        return true;
      }
      await page.getByTestId('session-detail-refresh').click().catch(() => {});
      return false;
    },
    Boolean,
    options.connectTimeoutMs,
    1000,
  );
}

async function waitForStopActionEnabled(page, options) {
  await poll(
    'session stop action',
    async () => {
      const enabled = await page.getByTestId('session-stop').isEnabled().catch(() => false);
      if (enabled) {
        return true;
      }
      await page.getByTestId('session-detail-refresh').click().catch(() => {});
      return false;
    },
    Boolean,
    options.connectTimeoutMs,
    1000,
  );
}

async function waitForPreviewConnected(popup, options) {
  const status = await poll(
    'session preview popup connection',
    async () => await popup.getByTestId('session-preview-status').textContent().catch(() => ''),
    (value) => value === 'Connected' || value === 'Connection failed',
    options.connectTimeoutMs,
  );
  if (status === 'Connection failed') {
    const detail = await popup.getByTestId('session-preview-error').textContent().catch(() => '');
    throw new Error(`Session preview popup connection failed${detail ? `: ${detail}` : ''}`);
  }
}

async function assertPreviewResizeUsesIndependentHeight(popup, options) {
  await popup.setViewportSize({ width: 1100, height: 760 });
  const size = await poll(
    'session preview popup independent resize',
    async () => await popup.getByTestId('session-preview-viewport').evaluate((element) => {
      const canvas = element.querySelector('canvas');
      const rect = element.getBoundingClientRect();
      const parentRect = element.parentElement?.getBoundingClientRect();
      const scale = window.devicePixelRatio || 1;
      return {
        canvasWidth: canvas?.width ?? 0,
        canvasHeight: canvas?.height ?? 0,
        viewportWidth: Math.round(rect.width * scale),
        viewportHeight: Math.round(rect.height * scale),
        parentViewportHeight: parentRect ? Math.round(parentRect.height * scale) : 0,
      };
    }),
    (value) => {
      if (
        !value.canvasWidth
        || !value.canvasHeight
        || !value.viewportWidth
        || !value.viewportHeight
        || !value.parentViewportHeight
      ) {
        return false;
      }
      const matchesViewport = Math.abs(value.canvasWidth - value.viewportWidth) <= 2
        && Math.abs(value.canvasHeight - value.viewportHeight) <= 2;
      const fillsPreviewSection = Math.abs(value.viewportHeight - value.parentViewportHeight) <= 2;
      const fixedRatioHeight = Math.round(value.canvasWidth * 9 / 16);
      return matchesViewport && fillsPreviewSection && Math.abs(value.canvasHeight - fixedRatioHeight) > 10;
    },
    options.connectTimeoutMs,
    250,
  );
  if (!size) {
    throw new Error('Session preview popup canvas did not resize independently.');
  }
}

async function waitForSessionDetailUrl(page, options, expectedSessionId = '') {
  const result = await poll(
    'created unified session detail route',
    async () => {
      const url = new URL(page.url());
      const match = url.pathname.match(/\/admin-new\/sessions\/([^/]+)$/);
      const sessionId = match?.[1] ? decodeURIComponent(match[1]) : '';
      const error = await page.getByTestId('session-create-error').textContent().catch(() => '');
      const detailVisible = await page.getByTestId('session-detail-route').isVisible().catch(() => false);
      return {
        sessionId: sessionId && sessionId !== 'new' ? sessionId : '',
        detailVisible,
        error,
        url: url.toString(),
      };
    },
    (value) => value.detailVisible || Boolean(value.error),
    options.connectTimeoutMs,
  );
  if (result.error) {
    throw new Error(`Session create form failed: ${result.error}`);
  }
  if (!result.sessionId) {
    throw new Error(`Created session detail rendered at an unexpected URL: ${result.url}`);
  }
  if (expectedSessionId && result.sessionId !== expectedSessionId) {
    throw new Error(`Expected created session detail ${expectedSessionId}, got ${result.sessionId}`);
  }
  return result.sessionId;
}

async function cleanupStaleSessionSmokes(accessToken, options, log) {
  const catalog = await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  const sessionIds = findActiveSessionIdsByLabels(catalog, {
    suite: 'admin-unified-sessions',
    purpose: 'smoke',
  });
  for (const sessionId of sessionIds) {
    await cleanupSession(accessToken, options, sessionId);
  }
  if (sessionIds.length > 0) {
    log(`Removed ${sessionIds.length} stale admin-new session smoke runtime(s).`);
  }
}

async function cleanupSession(accessToken, options, sessionId) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(sessionId)}/kill`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (response.ok || response.status === 404 || response.status === 409) {
    return;
  }
  const detail = await response.text().catch(() => '');
  throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
}

function shortSessionId(sessionId) {
  return sessionId.length > 13 ? `${sessionId.slice(0, 8)}...${sessionId.slice(-4)}` : sessionId;
}

run().catch((error) => {
  console.error(`[admin-unified-sessions-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
