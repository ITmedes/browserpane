import assert from 'node:assert/strict';
import process from 'node:process';
import { chromium } from 'playwright-core';
import { ensureAdminLoggedIn, getAdminAccessToken } from './admin-smoke-lib.mjs';
import {
  adminRouteUrl,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  authJsonHeaders,
  waitForContains,
} from './admin-unified-smoke-lib.mjs';
import {
  DEFAULTS,
  apiOrigin,
  createLogger,
  launchChrome,
  parseSmokeArgs,
} from './workflow-smoke-lib.mjs';

const EXPECTED_CLASSIFICATIONS = Object.freeze({
  'ui-primary': 108,
  'ui-evidence': 6,
  'api-companion': 5,
  'internal-worker': 12,
});

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-unified-api-companion-smoke.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin-new/api`;
  }
  const log = createLogger('admin-unified-api-companion-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();
  const origin = apiOrigin(options);
  const runLabel = `api-companion-${Date.now()}`;
  let accessToken = '';
  let project = null;
  let session = null;

  try {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    accessToken = await getAdminAccessToken(page);
    assert.ok(accessToken, 'Expected an owner access token after Keycloak login.');

    const evidence = await loadPublishedEvidence(origin);
    assertEvidence(evidence);
    await verifyApiCompanion(page, options, accessToken);
    await verifyCoverage(page, options);
    await verifyDocs(page, options, evidence.compatibility.surfaces.length);
    await verifyResponsiveRoutes(page, options);
    await verifyUnauthorizedExamples(origin, evidence.examples.examples);

    project = await createProject(origin, accessToken, evidence.examples.examples, runLabel);
    session = await createSession(origin, accessToken, evidence.examples.examples, project.id, runLabel);
    const ticket = await issueConnectTicket(origin, accessToken, session.id);
    assert.equal(ticket.token_type, 'session_connect_ticket');
    assert.equal(ticket.session_id, session.id);
    assert.equal(typeof ticket.token, 'string');
    assert.ok(ticket.token.length > 10);

    const workflows = await ownerRequest(origin, accessToken, '/api/v1/workflows');
    assert.ok(Array.isArray(workflows.workflows), 'Expected workflow list response.');
    const workspaces = await ownerRequest(origin, accessToken, '/api/v1/file-workspaces');
    assert.ok(Array.isArray(workspaces.workspaces), 'Expected file-workspace list response.');

    assert.equal((await page.locator('body').textContent()).includes(accessToken), false);
    console.log(JSON.stringify({
      operations: evidence.operations.operations.length,
      classifications: EXPECTED_CLASSIFICATIONS,
      compatibilitySurfaces: evidence.compatibility.surfaces.length,
      projectId: project.id,
      sessionId: session.id,
      connectTicketIssued: true,
      workflowListExecuted: true,
      fileWorkspaceListExecuted: true,
      tokenRendered: false,
    }, null, 2));
  } finally {
    if (accessToken && session?.id) {
      await bestEffortRequest(origin, accessToken, `/api/v1/sessions/${encodeURIComponent(session.id)}/stop`, { method: 'POST' }, log);
    }
    if (accessToken && project?.id) {
      await archiveProject(origin, accessToken, project, log);
    }
    await context.close();
    await browser.close();
  }
}

async function loadPublishedEvidence(origin) {
  const [operations, classifications, examples, compatibility, openapi] = await Promise.all([
    fetchDocument(origin, '/openapi/bpane-control-v1.operations.json'),
    fetchDocument(origin, '/openapi/bpane-control-v1.classifications.json'),
    fetchDocument(origin, '/openapi/bpane-control-v1.examples.json'),
    fetchDocument(origin, '/openapi/bpane-control-v1.compatibility.json'),
    fetchText(origin, '/openapi/bpane-control-v1.yaml'),
  ]);
  return { operations, classifications, examples, compatibility, openapi };
}

function assertEvidence(evidence) {
  assert.equal(evidence.operations.version, 1);
  assert.equal(evidence.operations.contract, 'bpane-control-v1');
  assert.equal(evidence.operations.operations.length, 131);
  assert.equal(evidence.examples.examples.length, 19);
  assert.equal(evidence.compatibility.surfaces.length, 14);
  assert.match(evidence.openapi, /title: BrowserPane Control Plane API/);
  for (const [classification, count] of Object.entries(EXPECTED_CLASSIFICATIONS)) {
    assert.equal(evidence.classifications.classifications[classification].length, count);
    assert.equal(
      evidence.operations.operations.filter((operation) => operation.classification === classification).length,
      count,
    );
  }
}

async function verifyApiCompanion(page, options, accessToken) {
  await page.goto(adminRouteUrl(options, 'api'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('api-companion-workspace').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await waitForContains(page, options, 'api-summary-total', '131');
  for (const id of ['projects', 'sessions', 'workflows', 'file-workspaces']) {
    await page.getByTestId(`api-task-${id}`).waitFor({ state: 'visible' });
  }
  const command = page.getByTestId('api-command-companion-project-create');
  const commandText = await command.textContent();
  assert.match(commandText, /BPANE_OWNER_TOKEN/);
  assert.doesNotMatch(commandText, new RegExp(escapeRegex(accessToken)));
  await page.getByTestId('api-command-companion-project-create-copy').click();
  await waitForContains(page, options, 'api-command-companion-project-create-feedback', 'copied');
  assert.equal(await page.getByTestId('api-download-openapi').getAttribute('href'), '/openapi/bpane-control-v1.yaml');
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('api-companion-workspace').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await assertNoHorizontalOverflow(page, 'api-companion-workspace', 'API companion workspace');
  await assertNoBodyHorizontalOverflow(page, 'API companion route');
}

async function verifyCoverage(page, options) {
  await page.goto(adminRouteUrl(options, 'coverage'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('api-coverage-workspace').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  assert.equal(await page.getByTestId('api-operation-row').count(), 131);
  for (const [classification, count] of Object.entries(EXPECTED_CLASSIFICATIONS)) {
    await page.getByTestId('api-coverage-classification').selectOption(classification);
    await waitForContains(page, options, 'api-coverage-result-count', `${count} of 131`);
    assert.equal(await page.getByTestId('api-operation-row').count(), count);
  }
  await page.getByTestId('api-coverage-clear').click();
  await waitForContains(page, options, 'api-coverage-result-count', '131 of 131');
  await page.goto(`${adminRouteUrl(options, 'coverage')}?operation=createProject`, { waitUntil: 'domcontentloaded' });
  await waitForContains(page, options, 'api-coverage-result-count', '1 of 131');
  assert.equal(await page.getByTestId('api-operation-row').getAttribute('data-operation-id'), 'createProject');
  await page.reload({ waitUntil: 'domcontentloaded' });
  await waitForContains(page, options, 'api-coverage-result-count', '1 of 131');
  await assertNoBodyHorizontalOverflow(page, 'API coverage route');
}

async function verifyDocs(page, options, expectedCompatibilityCount) {
  await page.goto(adminRouteUrl(options, 'docs'), { waitUntil: 'domcontentloaded' });
  await page.getByTestId('admin-docs-workspace').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  assert.equal(await page.getByTestId('docs-compatibility-row').count(), expectedCompatibilityCount);
  await waitForContains(page, options, 'docs-conformance', 'Semantic compatibility');
  await waitForContains(page, options, 'docs-compatibility-boundary', 'not frozen v1');
  assert.equal(await page.getByTestId('docs-openapi-download').getAttribute('href'), '/openapi/bpane-control-v1.yaml');
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.getByTestId('admin-docs-workspace').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
  await assertNoBodyHorizontalOverflow(page, 'admin docs route');
}

async function verifyResponsiveRoutes(page, options) {
  await page.setViewportSize({ width: 390, height: 844 });
  for (const [route, testId] of [
    ['api', 'api-companion-workspace'],
    ['coverage', 'api-coverage-workspace'],
    ['docs', 'admin-docs-workspace'],
  ]) {
    await page.goto(adminRouteUrl(options, route), { waitUntil: 'domcontentloaded' });
    await page.getByTestId(testId).waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await assertNoBodyHorizontalOverflow(page, `${route} mobile route`);
  }
  await page.setViewportSize({ width: 1440, height: 980 });
}

async function verifyUnauthorizedExamples(origin, examples) {
  const names = [
    'companion-project-create',
    'companion-session-create',
    'companion-session-connect-ticket',
    'companion-workflow-run-create',
    'companion-file-workspace-create',
  ];
  for (const name of names) {
    const example = examples.find((item) => item.name === name);
    assert.ok(example, `Missing published example ${name}.`);
    const response = await fetch(new URL(example.request.path, origin), {
      method: example.request.method,
      headers: example.request.body === undefined ? undefined : { 'content-type': 'application/json' },
      body: example.request.body === undefined ? undefined : JSON.stringify(example.request.body),
    });
    assert.equal(response.status, 401, `${name} should reject missing owner authentication.`);
  }
}

async function createProject(origin, accessToken, examples, runLabel) {
  const example = requiredExample(examples, 'companion-project-create');
  return await requestJson(origin, example.request.path, {
    method: example.request.method,
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      ...example.request.body,
      name: `API companion ${runLabel}`,
      labels: { suite: 'admin-unified-api-companion', run: runLabel },
    }),
  }, 201);
}

async function createSession(origin, accessToken, examples, projectId, runLabel) {
  const example = requiredExample(examples, 'companion-session-create');
  return await requestJson(origin, example.request.path, {
    method: example.request.method,
    headers: authJsonHeaders(accessToken),
    body: JSON.stringify({
      ...example.request.body,
      project_id: projectId,
      labels: { suite: 'admin-unified-api-companion', run: runLabel },
    }),
  }, 201);
}

async function issueConnectTicket(origin, accessToken, sessionId) {
  return await requestJson(
    origin,
    `/api/v1/sessions/${encodeURIComponent(sessionId)}/access-tokens`,
    { method: 'POST', headers: authJsonHeaders(accessToken) },
    200,
  );
}

async function ownerRequest(origin, accessToken, path) {
  return await requestJson(origin, path, { headers: authJsonHeaders(accessToken) }, 200);
}

async function archiveProject(origin, accessToken, project, log) {
  await bestEffortRequest(origin, accessToken, `/api/v1/projects/${encodeURIComponent(project.id)}`, {
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
  }, log);
}

async function bestEffortRequest(origin, accessToken, path, init, log) {
  try {
    const response = await fetch(new URL(path, origin), {
      ...init,
      headers: init.headers ?? authJsonHeaders(accessToken),
    });
    if (!response.ok && response.status !== 409) {
      log(`Cleanup ${path} returned HTTP ${response.status}.`);
    }
  } catch (error) {
    log(`Cleanup ${path} failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

async function requestJson(origin, path, init, expectedStatus) {
  const response = await fetch(new URL(path, origin), init);
  const text = await response.text();
  assert.equal(response.status, expectedStatus, `${init.method ?? 'GET'} ${path}: ${text}`);
  return text ? JSON.parse(text) : null;
}

async function fetchDocument(origin, path) {
  return JSON.parse(await fetchText(origin, path));
}

async function fetchText(origin, path) {
  const response = await fetch(new URL(path, origin));
  assert.equal(response.status, 200, `Expected ${path} to be published.`);
  return await response.text();
}

function requiredExample(examples, name) {
  const example = examples.find((item) => item.name === name);
  assert.ok(example, `Missing published example ${name}.`);
  return example;
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

run().catch((error) => {
  console.error(`[admin-unified-api-companion-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
