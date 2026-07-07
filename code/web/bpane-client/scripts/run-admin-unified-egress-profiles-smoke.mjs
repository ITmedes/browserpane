import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  ensureAdminLoggedIn,
  getAdminAccessToken,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-egress-profiles-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/egress`;
  }
  const log = createLogger('admin-unified-egress-profiles-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const runLabel = `admin-unified-egress-smoke-${Date.now()}`;
  let accessToken = '';
  let project = null;
  let directProfile = null;
  let tlsProfile = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    if (!accessToken) {
      throw new Error('No admin access token available after login.');
    }

    project = await createProject(accessToken, options, runLabel);
    const credentialBinding = await createProxyCredentialBinding(accessToken, options, project, runLabel);

    await page.goto(adminRouteUrl(options, 'egress'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('egress-profiles-new-link').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoBodyHorizontalOverflow(page, 'unified egress catalog');
    await assertNoHorizontalOverflow(page, 'egress-profiles-overview', 'unified egress overview');

    directProfile = await createDirectProfileThroughUi(page, accessToken, options, runLabel);
    verifyDirectProfile(directProfile, runLabel);
    await verifyCatalogSearch(page, options, directProfile.name, directProfile.id);

    tlsProfile = await createTlsProfileThroughUi(page, accessToken, options, {
      project,
      credentialBinding,
      runLabel,
    });
    verifyTlsProfile(tlsProfile, project, credentialBinding, runLabel);

    tlsProfile = await updateTlsProfileThroughUi(page, accessToken, options, tlsProfile, runLabel);
    verifyUpdatedTlsProfile(tlsProfile, project, credentialBinding, runLabel);
    await verifyCatalogSearch(page, options, tlsProfile.name, tlsProfile.id);
    await verifyResponsiveDetailLayout(page, options, tlsProfile.id);

    console.log(JSON.stringify({
      directProfileId: directProfile.id,
      directProfileName: directProfile.name,
      tlsProfileId: tlsProfile.id,
      tlsProfileName: tlsProfile.name,
      projectId: project.id,
      credentialBindingId: credentialBinding.id,
    }, null, 2));
  } finally {
    if (accessToken && directProfile) {
      await disableProfile(accessToken, options, directProfile).catch((error) => {
        log(`Direct profile cleanup for ${directProfile.id} failed: ${error.message}`);
      });
    }
    if (accessToken && tlsProfile) {
      await disableProfile(accessToken, options, tlsProfile).catch((error) => {
        log(`TLS profile cleanup for ${tlsProfile.id} failed: ${error.message}`);
      });
    }
    if (accessToken && project) {
      await archiveProject(accessToken, options, project).catch((error) => {
        log(`Project cleanup for ${project.id} failed: ${error.message}`);
      });
    }
    await context.close();
    await browser.close();
  }
}

async function createDirectProfileThroughUi(page, accessToken, options, runLabel) {
  await page.goto(adminRouteUrl(options, 'egress/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('egress-profile-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'unified egress create route');
  await assertNoHorizontalOverflow(page, 'egress-profile-create-route', 'unified egress create route');
  await verifyCreateValidation(page);

  await page.goto(adminRouteUrl(options, 'egress/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('egress-profile-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('egress-profile-edit-name').fill(`Unified direct egress ${runLabel}`);
  await page.getByTestId('egress-profile-edit-description').fill('Created by the unified admin egress smoke.');
  await page.getByTestId('egress-profile-edit-labels').fill(`suite=admin-unified-egress-smoke\nrun=${runLabel}`);
  await page.getByTestId('egress-profile-edit-save').click();

  const profileId = await waitForProfileDetailNavigation(page, options);
  const profile = await fetchProfile(accessToken, options, profileId);
  await page.getByTestId('egress-profile-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  return profile;
}

async function createTlsProfileThroughUi(page, accessToken, options, request) {
  await page.goto(adminRouteUrl(options, 'egress/new'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('egress-profile-create-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });

  await page.getByTestId('egress-profile-edit-name').fill(`Unified TLS egress ${request.runLabel}`);
  await page.getByTestId('egress-profile-edit-description').fill('Project-scoped TLS egress profile created by smoke.');
  await page.getByTestId('egress-profile-edit-labels').fill(
    `suite=admin-unified-egress-smoke\nrun=${request.runLabel}\nmode=tls`,
  );
  await page.getByTestId('egress-profile-edit-project-binding').selectOption('project');
  await page.locator(`[data-testid="egress-profile-edit-project-id"] option[value="${request.project.id}"]`).waitFor({
    state: 'attached',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('egress-profile-edit-project-id').selectOption(request.project.id);
  await page.getByTestId('egress-profile-edit-observation-mode').selectOption('tls_intercept');
  await page.getByTestId('egress-profile-edit-proxy-url').fill('http://bpane-egress-tls-observer:3129');
  await page.getByTestId('egress-profile-edit-proxy-credential-binding-id').fill(request.credentialBinding.id);
  await page.getByTestId('egress-profile-edit-bypass-rules').fill('localhost\n*.local\nlocalhost');
  await page.getByTestId('egress-profile-edit-custom-ca-certificate-ref').fill('file:///workspace/dev/egress-ca.pem');
  await page.getByTestId('egress-profile-edit-custom-ca-display-name').fill('BrowserPane Local Egress Test CA');
  await page.getByTestId('egress-profile-edit-sensitive-log-sink-ref').fill('siem://browserpane/local-egress');
  await page.getByTestId('egress-profile-edit-sensitive-log-sink-display-name').fill('Local Egress SIEM');
  await assertNoBodyHorizontalOverflow(page, 'unified egress TLS create route');
  await assertNoHorizontalOverflow(page, 'egress-profile-edit-form', 'unified egress TLS edit form');

  if (!await page.getByTestId('egress-profile-edit-save').isEnabled()) {
    throw new Error('Expected TLS egress profile save to be enabled after all required fields were filled.');
  }
  await page.getByTestId('egress-profile-edit-save').click();

  const profileId = await waitForProfileDetailNavigation(page, options);
  return await fetchProfile(accessToken, options, profileId);
}

async function updateTlsProfileThroughUi(page, accessToken, options, profile, runLabel) {
  await page.goto(adminRouteUrl(options, `egress/${profile.id}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('egress-profile-detail-route').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('egress-profile-edit-form').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertInputValue(page, 'egress-profile-edit-name', `Unified TLS egress ${runLabel}`);
  await assertInputValue(page, 'egress-profile-edit-project-id', profile.project_id);
  await assertInputValue(page, 'egress-profile-edit-proxy-url', 'http://bpane-egress-tls-observer:3129');
  await assertInputValue(page, 'egress-profile-edit-custom-ca-certificate-ref', 'file:///workspace/dev/egress-ca.pem');

  if (!await page.getByTestId('egress-profile-edit-save').isDisabled()) {
    throw new Error('Expected save to be disabled before editing an existing egress profile.');
  }

  await page.getByTestId('egress-profile-edit-name').fill(`Unified TLS egress updated ${runLabel}`);
  await page.getByTestId('egress-profile-edit-description').fill('Updated through the unified admin egress smoke.');
  await page.getByTestId('egress-profile-edit-state').selectOption('disabled');
  await page.getByTestId('egress-profile-edit-labels').fill(
    `suite=admin-unified-egress-smoke\nrun=${runLabel}\nmode=tls\nphase=updated`,
  );
  await page.getByTestId('egress-profile-edit-sensitive-log-sink-display-name').fill('Updated Local Egress SIEM');
  if (!await page.getByTestId('egress-profile-edit-save').isEnabled()) {
    throw new Error('Expected save to be enabled after editing the egress profile.');
  }
  await page.getByTestId('egress-profile-edit-save').click();
  await page.getByTestId('egress-profile-action-success').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  const successText = await page.getByTestId('egress-profile-action-success').textContent();
  if (!successText?.includes('Egress profile saved')) {
    throw new Error(`Expected egress profile save success message, got ${successText}`);
  }
  return await fetchProfile(accessToken, options, profile.id);
}

async function verifyCreateValidation(page) {
  if (!await page.getByTestId('egress-profile-edit-save').isDisabled()) {
    throw new Error('Expected new egress profile save to be disabled before required fields are entered.');
  }
  await page.getByTestId('egress-profile-edit-name').fill('Invalid labels profile');
  await page.getByTestId('egress-profile-edit-labels').fill('broken-label');
  const labelsError = await page.getByTestId('egress-profile-edit-labels-error').textContent();
  if (!labelsError?.includes('key=value')) {
    throw new Error(`Expected label validation error, got ${labelsError}`);
  }
  if (!await page.getByTestId('egress-profile-edit-save').isDisabled()) {
    throw new Error('Expected save to stay disabled while labels are invalid.');
  }

  await page.getByTestId('egress-profile-edit-labels').fill('');
  await page.getByTestId('egress-profile-edit-observation-mode').selectOption('tls_intercept');
  const proxyError = await page.getByTestId('egress-profile-edit-proxy-url-error').textContent();
  const caError = await page.getByTestId('egress-profile-edit-custom-ca-certificate-ref-error').textContent();
  const sinkError = await page.getByTestId('egress-profile-edit-sensitive-log-sink-ref-error').textContent();
  if (!proxyError?.includes('Proxy URL is required')) {
    throw new Error(`Expected proxy URL validation error, got ${proxyError}`);
  }
  if (!caError?.includes('Custom CA certificate reference is required')) {
    throw new Error(`Expected custom CA validation error, got ${caError}`);
  }
  if (!sinkError?.includes('TLS intercept requires')) {
    throw new Error(`Expected sensitive log sink validation error, got ${sinkError}`);
  }
  if (!await page.getByTestId('egress-profile-edit-save').isDisabled()) {
    throw new Error('Expected save to stay disabled while TLS requirements are incomplete.');
  }
  await assertNoBodyHorizontalOverflow(page, 'unified egress validation messages');
  await assertNoHorizontalOverflow(page, 'egress-profile-edit-form', 'unified egress validation form');
}

async function verifyCatalogSearch(page, options, profileName, profileId) {
  await page.goto(adminRouteUrl(options, 'egress'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('egress-profiles-new-link').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await page.getByTestId('egress-profiles-search').fill(profileName);
  const row = page.locator('[data-testid="egress-profiles-list-row"]').filter({ hasText: profileName });
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const link = row.getByTestId('egress-profiles-detail-link');
  const href = await link.getAttribute('href');
  if (!href?.endsWith(`/admin-new/egress/${encodeURIComponent(profileId)}`)) {
    throw new Error(`Expected catalog detail link for ${profileId}, got ${href}`);
  }
  await assertNoBodyHorizontalOverflow(page, 'unified egress filtered catalog');
}

async function verifyResponsiveDetailLayout(page, options, profileId) {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.goto(adminRouteUrl(options, `egress/${profileId}`), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('egress-profile-edit-form').waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await assertNoBodyHorizontalOverflow(page, 'mobile unified egress detail route');
  await assertNoHorizontalOverflow(page, 'egress-profile-detail-route', 'mobile unified egress detail route');
  await assertNoHorizontalOverflow(page, 'egress-profile-edit-form', 'mobile unified egress edit form');
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function waitForProfileDetailNavigation(page, options) {
  await page.waitForURL((url) => {
    const expectedPrefix = new URL('/admin-new/egress/', apiOrigin(options)).toString();
    return url.toString().startsWith(expectedPrefix) && !url.pathname.endsWith('/new');
  }, { timeout: options.connectTimeoutMs });
  const profileId = decodeURIComponent(new URL(page.url()).pathname.split('/').filter(Boolean).at(-1) ?? '');
  if (!profileId || profileId === 'new') {
    throw new Error(`Expected create flow to navigate to egress detail route, got ${page.url()}`);
  }
  return profileId;
}

async function createProject(accessToken, options, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified egress smoke project ${runLabel}`,
      description: 'Project binding target for the unified admin egress smoke.',
      labels: { suite: 'admin-unified-egress-smoke', run: runLabel },
      state: 'active',
      policy: {
        allowed_session_template_ids: [],
        allowed_egress_profile_ids: [],
        allowed_extension_ids: [],
        allowed_browser_context_ids: [],
        allowed_file_workspace_ids: [],
        allow_browser_uploads: true,
        allow_browser_downloads: true,
        allow_session_file_bindings: true,
        allow_manual_recordings: true,
        usage_budget_enforcement: 'warning_only',
      },
      quotas: {},
    }),
  });
}

async function createProxyCredentialBinding(accessToken, options, project, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/credential-bindings`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Unified egress proxy auth ${runLabel}`,
      provider: 'vault_kv_v2',
      allowed_origins: ['http://bpane-egress-tls-observer:3129'],
      injection_mode: 'form_fill',
      secret_payload: {
        username: 'proxy-user',
        password: 'proxy-pass',
      },
      labels: {
        suite: 'admin-unified-egress-smoke',
        run: runLabel,
      },
      project_id: project.id,
    }),
  });
}

async function fetchProfile(accessToken, options, profileId) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles/${profileId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function disableProfile(accessToken, options, profile) {
  const body = profileRequest(profile, { state: 'disabled' });
  return await fetchJson(`${apiOrigin(options)}/api/v1/egress-profiles/${profile.id}`, {
    method: 'PUT',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify(body),
  });
}

async function archiveProject(accessToken, options, project) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects/${project.id}`, {
    method: 'PUT',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: project.name,
      description: project.description,
      labels: project.labels ?? {},
      quotas: project.quotas ?? {},
      policy: project.policy ?? {},
      state: 'archived',
    }),
  });
}

function verifyDirectProfile(profile, runLabel) {
  assertEqual(profile.name, `Unified direct egress ${runLabel}`, 'direct profile name');
  assertEqual(profile.description, 'Created by the unified admin egress smoke.', 'direct profile description');
  assertEqual(profile.project_id, null, 'direct profile project id');
  assertEqual(profile.proxy, null, 'direct profile proxy');
  assertEqual(profile.custom_ca, null, 'direct profile custom CA');
  assertEqual(profile.traffic_observation?.mode, 'metadata_only', 'direct profile observation mode');
  assertEqual(profile.labels?.suite, 'admin-unified-egress-smoke', 'direct profile suite label');
  assertEqual(profile.labels?.run, runLabel, 'direct profile run label');
}

function verifyTlsProfile(profile, project, credentialBinding, runLabel) {
  assertEqual(profile.name, `Unified TLS egress ${runLabel}`, 'TLS profile name');
  assertEqual(profile.description, 'Project-scoped TLS egress profile created by smoke.', 'TLS profile description');
  assertEqual(profile.project_id, project.id, 'TLS profile project id');
  assertEqual(profile.proxy?.url, 'http://bpane-egress-tls-observer:3129', 'TLS profile proxy URL');
  assertEqual(profile.proxy?.credential_binding_id, credentialBinding.id, 'TLS profile credential binding id');
  assertEqual(profile.custom_ca?.certificate_ref, 'file:///workspace/dev/egress-ca.pem', 'TLS profile custom CA ref');
  assertEqual(profile.custom_ca?.display_name, 'BrowserPane Local Egress Test CA', 'TLS profile custom CA name');
  assertEqual(profile.traffic_observation?.mode, 'tls_intercept', 'TLS profile observation mode');
  assertEqual(profile.traffic_observation?.sensitive_log_sink_ref, 'siem://browserpane/local-egress', 'TLS profile sink ref');
  assertEqual(profile.traffic_observation?.sensitive_log_sink_display_name, 'Local Egress SIEM', 'TLS profile sink name');
  assertSameIds(profile.bypass_rules, ['*.local', 'localhost'], 'TLS profile bypass rules');
  assertEqual(profile.labels?.suite, 'admin-unified-egress-smoke', 'TLS profile suite label');
  assertEqual(profile.labels?.run, runLabel, 'TLS profile run label');
  assertEqual(profile.labels?.mode, 'tls', 'TLS profile mode label');
}

function verifyUpdatedTlsProfile(profile, project, credentialBinding, runLabel) {
  assertEqual(profile.name, `Unified TLS egress updated ${runLabel}`, 'updated TLS profile name');
  assertEqual(profile.description, 'Updated through the unified admin egress smoke.', 'updated TLS profile description');
  assertEqual(profile.state, 'disabled', 'updated TLS profile state');
  verifyTlsProfile(
    {
      ...profile,
      name: `Unified TLS egress ${runLabel}`,
      description: 'Project-scoped TLS egress profile created by smoke.',
      state: 'ready',
      traffic_observation: {
        ...profile.traffic_observation,
        sensitive_log_sink_display_name: 'Local Egress SIEM',
      },
    },
    project,
    credentialBinding,
    runLabel,
  );
  assertEqual(profile.labels?.phase, 'updated', 'updated TLS profile phase label');
  assertEqual(profile.traffic_observation?.sensitive_log_sink_display_name, 'Updated Local Egress SIEM', 'updated TLS profile sink name');
}

function profileRequest(profile, overrides = {}) {
  return {
    project_id: profile.project_id ?? null,
    name: profile.name,
    description: profile.description ?? null,
    labels: profile.labels ?? {},
    proxy: profile.proxy ?? null,
    bypass_rules: profile.bypass_rules ?? [],
    custom_ca: profile.custom_ca ?? null,
    traffic_observation: profile.traffic_observation ?? { mode: 'metadata_only' },
    state: profile.state ?? 'ready',
    ...overrides,
  };
}

async function assertNoHorizontalOverflow(page, testId, label) {
  const size = await page.getByTestId(testId).evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
  }));
  if (size.scrollWidth > size.clientWidth + 1) {
    throw new Error(`${label} overflows horizontally: ${JSON.stringify(size)}`);
  }
}

async function assertNoBodyHorizontalOverflow(page, label) {
  const size = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  if (size.scrollWidth > size.clientWidth + 1) {
    throw new Error(`${label} causes document horizontal overflow: ${JSON.stringify(size)}`);
  }
}

async function assertInputValue(page, testId, expected) {
  const actual = await page.getByTestId(testId).inputValue();
  assertEqual(actual, expected, `${testId} value`);
}

function adminRouteUrl(options, path) {
  return new URL(`/admin-new/${path.replace(/^\/+/, '')}`, apiOrigin(options)).toString();
}

function authJsonHeaders(accessToken) {
  return {
    Authorization: `Bearer ${accessToken}`,
    'content-type': 'application/json',
  };
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`Expected ${label} to be ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertSameIds(actual, expected, label) {
  const actualIds = [...(actual ?? [])].sort();
  const expectedIds = [...expected].sort();
  if (JSON.stringify(actualIds) !== JSON.stringify(expectedIds)) {
    throw new Error(`Expected ${label} to be ${JSON.stringify(expectedIds)}, got ${JSON.stringify(actualIds)}`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-egress-profiles-smoke] ${error.stack || error.message}`);
  process.exit(1);
});
