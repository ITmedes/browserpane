import assert from 'node:assert/strict';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn, getAdminAccessToken } from './admin-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  authJsonHeaders,
} from './admin-unified-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-identity-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/identity`;
  }
  const log = createLogger('admin-unified-identity-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const runLabel = `identity-smoke-${Date.now()}`;
  let accessToken = '';
  let project = null;
  let servicePrincipal = null;
  let mapping = null;
  let session = null;

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    project = await createProject(accessToken, options, runLabel);

    await page.goto(adminRouteUrl(options, 'identity'), { waitUntil: 'domcontentloaded' });
    await page.getByTestId('identity-access-workspace').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await page.getByTestId('identity-principal-name').waitFor({
      state: 'visible',
      timeout: options.connectTimeoutMs,
    });
    await assertNoHorizontalOverflow(page, 'identity-access-workspace', 'identity access workspace');
    await assertNoBodyHorizontalOverflow(page, 'identity desktop route');
    await assertSanitizedDocument(page, accessToken);

    servicePrincipal = await createAndExerciseServicePrincipal(page, options, project, runLabel);
    mapping = await createAndExerciseMapping(page, options, project, servicePrincipal, runLabel);
    session = await createDelegatedSession(accessToken, options, project.id, servicePrincipal, runLabel);
    await verifyDelegationReview(page, options, servicePrincipal.client_id);
    await verifyRefreshPersistence(page, options, servicePrincipal, mapping, runLabel);
    await verifyResponsiveLayout(page);

    console.log(JSON.stringify({
      projectId: project.id,
      servicePrincipalId: servicePrincipal.id,
      identityMappingId: mapping.id,
      delegatedSessionId: session.id,
      sanitized: true,
      deepLinkRefresh: true,
    }, null, 2));
  } finally {
    if (accessToken && session?.id) {
      await clearDelegation(accessToken, options, session.id, log);
      await stopSession(accessToken, options, session.id, log);
    }
    if (accessToken && mapping?.id) {
      await disableIdentityMapping(accessToken, options, mapping, log);
    }
    if (accessToken && servicePrincipal?.id) {
      await disableServicePrincipal(accessToken, options, servicePrincipal, log);
    }
    if (accessToken && project?.id) {
      await archiveProject(accessToken, options, project, log);
    }
    await context.close();
    await browser.close();
  }
}

async function createAndExerciseServicePrincipal(page, options, project, runLabel) {
  await page.getByTestId('identity-area-service-principals').click();
  await page.getByTestId('service-principal-catalog').waitFor({ state: 'visible' });
  await page.getByTestId('service-principal-create').click();
  await page.getByTestId('service-principal-editor').waitFor({ state: 'visible' });

  await page.getByTestId('service-principal-name').fill(`Workflow bridge ${runLabel}`);
  await page.getByTestId('service-principal-client-id').fill(`bpane-${runLabel}`);
  await page.getByTestId('service-principal-labels').fill('broken-label');
  await page.getByTestId('service-principal-labels-error').waitFor({ state: 'visible' });
  assert.equal(await page.getByTestId('service-principal-save').isDisabled(), true);
  await page.getByTestId('service-principal-labels').fill(`suite=admin-unified-identity\nrun=${runLabel}`);
  await page.getByTestId('service-principal-scopes').fill('session:delegate\nworkflow:invoke');
  const projectOption = page.locator(
    `[data-testid="service-principal-project-option"][data-project-id="${project.id}"]`,
  );
  await projectOption.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await projectOption.setChecked(true);

  let intercepted = false;
  const conflictHandler = async (route) => {
    if (route.request().method() !== 'POST') {
      await route.fallback();
      return;
    }
    intercepted = true;
    await route.fulfill({
      status: 409,
      contentType: 'application/json',
      body: JSON.stringify({
        error: 'The external identity registration changed.',
        code: 'service_principal_conflict',
        category: 'conflict',
        recovery_hint: 'Review the latest registration and retry.',
      }),
    });
  };
  await page.route('**/api/v1/service-principals', conflictHandler);
  try {
    await page.getByTestId('service-principal-save').click();
    const failure = page.getByTestId('identity-action-error');
    await failure.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    const message = await failure.textContent();
    assert.match(message ?? '', /external identity registration changed/i);
    assert.match(message ?? '', /Review the latest registration/i);
    await page.getByTestId('service-principal-editor').waitFor({ state: 'visible' });
  } finally {
    await page.unroute('**/api/v1/service-principals', conflictHandler);
  }
  assert.equal(intercepted, true, 'expected service-principal conflict interception');

  await page.getByTestId('service-principal-save').click();
  await page.getByTestId('service-principal-editor-shell').waitFor({
    state: 'hidden',
    timeout: options.connectTimeoutMs,
  });
  await waitForSuccess(page, options, 'Service principal registered');
  await page.getByTestId('service-principal-search').fill(runLabel);
  const row = page.locator('[data-testid="service-principal-row"]').filter({ hasText: runLabel });
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.getByTestId('service-principal-select').click();
  const servicePrincipalId = await row.locator('p.font-mono').last().textContent().catch(() => null);
  const review = await fetchAccessReview(page, options);
  const resource = review.service_principals.find((entry) => entry.client_id === `bpane-${runLabel}`);
  assert.ok(resource, `created service principal was missing; row hint ${servicePrincipalId}`);

  await page.getByTestId('service-principal-edit').click();
  await page.getByTestId('service-principal-description').fill('Updated through the admin-new identity smoke.');
  await page.getByTestId('service-principal-save').click();
  await page.getByTestId('service-principal-editor-shell').waitFor({
    state: 'hidden',
    timeout: options.connectTimeoutMs,
  });
  await waitForSuccess(page, options, 'Service principal updated');
  await page.getByTestId('service-principal-disable').click();
  await page.getByTestId('service-principal-enable').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('service-principal-enable').click();
  await page.getByTestId('service-principal-disable').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  return resource;
}

async function createAndExerciseMapping(page, options, project, servicePrincipal, runLabel) {
  await page.getByTestId('identity-area-mappings').click();
  await page.getByTestId('identity-mapping-catalog').waitFor({ state: 'visible' });
  await page.getByTestId('identity-mapping-create').click();
  await page.getByTestId('identity-mapping-kind').selectOption('service_principal');
  await page.getByTestId('identity-mapping-name').fill(`Workflow mapping ${runLabel}`);
  await page.getByTestId('identity-mapping-service-principal-id').selectOption(servicePrincipal.id);
  await page.getByTestId('identity-mapping-project-id').selectOption(project.id);
  await page.getByTestId('identity-mapping-labels').fill(`suite=admin-unified-identity\nrun=${runLabel}`);
  await page.getByTestId('identity-mapping-scopes').fill('session:delegate');
  await page.getByTestId('identity-mapping-save').click();
  await page.getByTestId('identity-mapping-editor-shell').waitFor({
    state: 'hidden',
    timeout: options.connectTimeoutMs,
  });
  await waitForSuccess(page, options, 'Identity mapping created');

  await page.getByTestId('identity-mapping-search').fill(runLabel);
  const row = page.locator('[data-testid="identity-mapping-row"]').filter({ hasText: runLabel });
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await row.getByTestId('identity-mapping-select').click();
  const review = await fetchAccessReview(page, options);
  const resource = review.identity_mappings.find((entry) => entry.name === `Workflow mapping ${runLabel}`);
  assert.ok(resource, 'created identity mapping was missing from access review');
  assert.equal(resource.service_principal_id, servicePrincipal.id);
  assert.equal(resource.project_id, project.id);

  await page.getByTestId('identity-mapping-edit').click();
  await page.getByTestId('identity-mapping-description').fill('Updated through the admin-new identity smoke.');
  await page.getByTestId('identity-mapping-save').click();
  await page.getByTestId('identity-mapping-editor-shell').waitFor({
    state: 'hidden',
    timeout: options.connectTimeoutMs,
  });
  await waitForSuccess(page, options, 'Identity mapping updated');
  await page.getByTestId('identity-mapping-disable').click();
  await page.getByTestId('identity-mapping-enable').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('identity-mapping-enable').click();
  await page.getByTestId('identity-mapping-disable').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  return resource;
}

async function createDelegatedSession(accessToken, options, projectId, servicePrincipal, runLabel) {
  const session = await fetchJson(`${apiOrigin(options)}/api/v1/sessions`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      project_id: projectId,
      labels: { suite: 'admin-unified-identity', run: runLabel },
    }),
  });
  await fetchJson(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(session.id)}/automation-owner`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      client_id: servicePrincipal.client_id,
      issuer: servicePrincipal.issuer,
      display_name: servicePrincipal.name,
    }),
  });
  return session;
}

async function verifyDelegationReview(page, options, clientId) {
  await page.getByTestId('identity-area-review').click();
  await page.getByTestId('identity-refresh').click();
  const row = page.locator('[data-testid="identity-delegation-row"]').filter({ hasText: clientId });
  await row.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  const text = await row.textContent();
  assert.match(text ?? '', /registered/i);
  assert.match(text ?? '', /1 \/ 1 active/i);
}

async function verifyRefreshPersistence(page, options, servicePrincipal, mapping, runLabel) {
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('identity-principal-name').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('identity-area-service-principals').click();
  await page.getByTestId('service-principal-search').fill(runLabel);
  await page.locator('[data-testid="service-principal-row"]').filter({ hasText: servicePrincipal.client_id })
    .waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await page.getByTestId('identity-area-mappings').click();
  await page.getByTestId('identity-mapping-search').fill(runLabel);
  await page.locator('[data-testid="identity-mapping-row"]').filter({ hasText: mapping.name })
    .waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
}

async function verifyResponsiveLayout(page) {
  await page.setViewportSize({ width: 390, height: 844 });
  await assertNoBodyHorizontalOverflow(page, 'identity mobile route');
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function assertSanitizedDocument(page, accessToken) {
  const evidence = await page.evaluate((token) => ({
    containsToken: document.body.innerText.includes(token),
    passwordFields: document.querySelectorAll('input[type="password"]').length,
    authorizationFields: document.querySelectorAll('input[name*="authorization" i]').length,
    secretFields: document.querySelectorAll('input[name*="secret" i]').length,
  }), accessToken);
  assert.deepEqual(evidence, {
    containsToken: false,
    passwordFields: 0,
    authorizationFields: 0,
    secretFields: 0,
  });
}

async function waitForSuccess(page, options, expected) {
  const feedback = page.getByTestId('identity-action-success');
  await feedback.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  assert.match(await feedback.textContent() ?? '', new RegExp(expected, 'i'));
}

async function fetchAccessReview(page, options) {
  const accessToken = await getAdminAccessToken(page);
  return await fetchJson(`${apiOrigin(options)}/api/v1/identity/access-review`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function createProject(accessToken, options, runLabel) {
  return await fetchJson(`${apiOrigin(options)}/api/v1/projects`, {
    method: 'POST',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: `Identity smoke project ${runLabel}`,
      description: 'Temporary project for admin-new identity smoke coverage.',
      labels: { suite: 'admin-unified-identity', run: runLabel },
      state: 'active',
    }),
  });
}

async function clearDelegation(accessToken, options, sessionId, log) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(sessionId)}/automation-owner`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    log(`Delegation cleanup returned HTTP ${response.status}.`);
  }
}

async function stopSession(accessToken, options, sessionId, log) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/sessions/${encodeURIComponent(sessionId)}/stop`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && ![404, 409, 410].includes(response.status)) {
    log(`Session cleanup returned HTTP ${response.status}.`);
  }
}

async function disableServicePrincipal(accessToken, options, resource, log) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/service-principals/${encodeURIComponent(resource.id)}`, {
    method: 'PUT',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: resource.name,
      description: resource.description,
      client_id: resource.client_id,
      issuer: resource.issuer,
      labels: resource.labels,
      scopes: resource.scopes,
      allowed_project_ids: resource.allowed_project_ids,
      state: 'disabled',
    }),
  });
  if (!response.ok) {
    log(`Service-principal cleanup returned HTTP ${response.status}.`);
  }
}

async function disableIdentityMapping(accessToken, options, resource, log) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/identity-mappings/${encodeURIComponent(resource.id)}`, {
    method: 'PUT',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: resource.name,
      description: resource.description,
      kind: resource.kind,
      issuer: resource.issuer,
      external_id: resource.external_id,
      claim_name: resource.claim_name,
      service_principal_id: resource.service_principal_id,
      project_id: resource.project_id,
      labels: resource.labels,
      scopes: resource.scopes,
      state: 'disabled',
    }),
  });
  if (!response.ok) {
    log(`Identity-mapping cleanup returned HTTP ${response.status}.`);
  }
}

async function archiveProject(accessToken, options, project, log) {
  const response = await fetch(`${apiOrigin(options)}/api/v1/projects/${encodeURIComponent(project.id)}`, {
    method: 'PUT',
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      name: project.name,
      description: project.description,
      labels: project.labels,
      quotas: project.quotas,
      policy: project.policy,
      state: 'archived',
    }),
  });
  if (!response.ok) {
    log(`Project cleanup returned HTTP ${response.status}.`);
  }
}

run().catch((error) => {
  console.error(`[admin-unified-identity-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
