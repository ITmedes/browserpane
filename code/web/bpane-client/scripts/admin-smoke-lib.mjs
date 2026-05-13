import { cleanupWorkflowSmokeSessions, fetchAuthConfig, fetchJson, poll } from './workflow-smoke-lib.mjs';

export async function ensureAdminLoggedIn(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'domcontentloaded' });
  const authConfig = await fetchAuthConfig(options);
  if (!authConfig || authConfig.mode !== 'oidc') {
    return authConfig;
  }
  const state = await waitForAdminAuthSurface(page, options);
  if (state.authenticated) {
    return authConfig;
  }
  if (!authConfig.exampleUser?.username || !authConfig.exampleUser?.password) {
    throw new Error('OIDC auth is enabled, but no example user is configured for smoke login.');
  }

  await page.getByTestId('admin-login').click();
  await fillKeycloakLogin(page, authConfig, options);
  await page.waitForURL(urlPatternFor(options.pageUrl), { timeout: options.connectTimeoutMs });
  await waitForAdminAuthenticated(page, options);
  return authConfig;
}

export async function cleanupAdminSmoke(page, options, log) {
  await cleanupAdminSession(page).catch(() => {});
  const accessToken = await getAdminAccessToken(page).catch(() => '');
  if (accessToken) {
    await cleanupWorkflowSmokeSessions(accessToken, rootApiOptions(options), log).catch(() => {});
  }
}

export async function cleanupAdminBeforeRun(page, options, log) {
  const accessToken = await getAdminAccessToken(page);
  if (accessToken) {
    await cleanupWorkflowSmokeSessions(accessToken, rootApiOptions(options), log);
  }
}

export async function waitForBrowserConnected(page, options) {
  const status = await poll(
    'admin embedded browser connection',
    async () => await page.getByTestId('browser-status').textContent(),
    (value) => value?.startsWith('Connected to') === true || value === 'Connection failed',
    options.connectTimeoutMs,
  );
  if (status === 'Connection failed') {
    const detail = await page.getByTestId('browser-error').textContent().catch(() => '');
    throw new Error(`Admin embedded browser connection failed${detail ? `: ${detail}` : ''}`);
  }
}

export async function disconnectEmbeddedBrowser(page, options) {
  await closeAdminOverlay(page);
  await page.getByTestId('browser-disconnect').click();
  await poll(
    'admin embedded browser disconnect',
    async () => await page.getByTestId('browser-status').textContent(),
    (status) => status === 'Disconnected',
    options.connectTimeoutMs,
  );
}

export async function waitForStopEnabled(page, options, sessionId) {
  await openAdminTab(page, 'lifecycle');
  await waitForSessionStopEligibility(page, options, sessionId);
  await page.getByTestId('session-detail-refresh').click();
  await poll('admin stop button enabled', async () => {
    return await page.getByTestId('session-stop').isEnabled();
  }, (enabled) => enabled, options.connectTimeoutMs);
}

export async function waitForKillEnabled(page, options, sessionId) {
  await openAdminTab(page, 'lifecycle');
  await waitForSessionClients(page, options, sessionId, 0);
  await page.getByTestId('session-detail-refresh').click();
  await poll('admin kill button enabled', async () => {
    return await page.getByTestId('session-kill').isEnabled();
  }, (enabled) => enabled, options.connectTimeoutMs);
}

export async function waitForSessionState(page, options, sessionId, expectedState) {
  await openAdminTab(page, 'lifecycle');
  await poll(`admin session API state ${expectedState}`, async () => {
    const session = await fetchSessionResource(page, options, sessionId);
    return session.state;
  }, (state) => state === expectedState, options.connectTimeoutMs);
  await page.getByTestId('session-detail-refresh').click();
  await poll(`admin session UI state ${expectedState}`, async () => {
    return await page.getByTestId('session-state').textContent();
  }, (state) => state === expectedState, options.connectTimeoutMs);
}

export async function openAdminTab(page, panelId) {
  await ensureAdminOverlayOpen(page);
  await page.getByTestId(`workspace-panel-toggle-${panelId}`).click();
  await page.getByTestId(`workspace-panel-${panelId}`).waitFor({ state: 'visible' });
}

export async function ensureAdminOverlayOpen(page) {
  if (await adminOverlayOpen(page)) {
    return;
  }
  await page.getByTestId('admin-overlay-open').click();
  await poll('admin overlay open', async () => await adminOverlayOpen(page), Boolean, 5000, 100);
}

export async function closeAdminOverlay(page) {
  if (!await adminOverlayOpen(page)) {
    return;
  }
  await page.getByTestId('admin-overlay-close').click();
  await poll('admin overlay closed', async () => await adminOverlayOpen(page), (open) => !open, 5000, 100);
}

async function adminOverlayOpen(page) {
  return await page.getByTestId('admin-overlay').getAttribute('data-admin-open').then((value) => value === 'true').catch(() => false);
}

async function fillKeycloakLogin(page, authConfig, options) {
  const username = page.locator('input[name="username"], #username').first();
  const password = page.locator('input[name="password"], #password').first();
  const loginState = await poll('admin OIDC login readiness', async () => ({
    authenticated: await adminAuthenticatedVisible(page),
    usernameVisible: await username.isVisible().catch(() => false),
  }), (value) => value.authenticated || value.usernameVisible, options.connectTimeoutMs);
  if (loginState.authenticated) {
    return;
  }

  await password.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await username.fill(authConfig.exampleUser.username);
  await password.fill(authConfig.exampleUser.password);
  await page.locator('input[type="submit"], #kc-login').click();
}

async function waitForAdminAuthSurface(page, options) {
  return await poll('admin auth surface', async () => ({
    login: await page.getByTestId('admin-login').isVisible().catch(() => false),
    authenticated: await adminAuthenticatedVisible(page),
  }), (state) => state.login || state.authenticated, options.connectTimeoutMs);
}

async function waitForAdminAuthenticated(page, options) {
  await poll(
    'admin authenticated route surface',
    async () => await adminAuthenticatedVisible(page),
    Boolean,
    options.connectTimeoutMs,
  );
}

async function adminAuthenticatedVisible(page) {
  return await page.getByTestId('session-new').isVisible().catch(() => false)
    || await page.getByTestId('session-inspector-new').isVisible().catch(() => false)
    || await page.getByTestId('file-workspace-create-submit').isVisible().catch(() => false);
}

async function waitForSessionClients(page, options, sessionId, expectedClients) {
  await poll(`admin session client count ${expectedClients}`, async () => {
    const session = await fetchSessionResource(page, options, sessionId);
    return session.status?.connection_counts?.total_clients;
  }, (count) => count === expectedClients, options.connectTimeoutMs);
}

async function waitForSessionStopEligibility(page, options, sessionId) {
  await poll('admin session stop eligibility', async () => {
    const session = await fetchSessionResource(page, options, sessionId);
    return {
      clients: session.status?.connection_counts?.total_clients,
      allowed: session.status?.stop_eligibility?.allowed,
    };
  }, (state) => state.clients === 0 && state.allowed === true, options.connectTimeoutMs);
}

async function fetchSessionResource(page, options, sessionId) {
  const accessToken = await getAdminAccessToken(page);
  return await fetchJson(`${rootApiOptions(options).pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function cleanupAdminSession(page) {
  if (await page.getByTestId('browser-disconnect').isEnabled().catch(() => false)) {
    await closeAdminOverlay(page);
    await page.getByTestId('browser-disconnect').click();
  }
  await openAdminTab(page, 'lifecycle').catch(() => {});
  if (await page.getByTestId('session-kill').isEnabled().catch(() => false)) {
    await page.getByTestId('session-kill').click();
  }
}

export async function getAdminAccessToken(page) {
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

function urlPatternFor(pageUrl) {
  const url = new URL(pageUrl);
  const prefix = `${url.origin}${url.pathname}`;
  return new RegExp(`^${prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`);
}
