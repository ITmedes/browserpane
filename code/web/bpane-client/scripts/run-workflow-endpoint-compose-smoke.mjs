#!/usr/bin/env node

import fs from 'node:fs/promises';
import process from 'node:process';

import { chromium } from 'playwright-core';

import { runWorkflowEndpointConformance } from './run-workflow-endpoint-conformance.mjs';
import {
  apiOrigin,
  buildWorkflowWorkerImage,
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLocalWorkflowRepo,
  createLogger,
  deleteSession,
  ensureLoggedIn,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

const log = createLogger('workflow-endpoint-compose-smoke');
const MACHINE_CLIENT_ID = 'bpane-fake-bpm';
const ENDPOINT_KEY = 'retrieve-supplier-report';
const ALL_OPERATIONS = ['invoke', 'read', 'cancel', 'artifact.read'];
const ALL_SCOPES = ALL_OPERATIONS.map((operation) => `workflow-endpoints:${operation}`);
const RUNTIME_TARGET_URL = 'http://web:8080/test-embed.html';

function workflowEntrypoint() {
  return `export default async function run({ page, input }) {
  await page.goto(input.target_url, { waitUntil: 'networkidle' });
  if (input.delay_ms) await page.waitForTimeout(input.delay_ms);
  if (input.invalid_output) return { unexpected: true };
  return {
    status: 'complete',
    reporting_period: input.reporting_period,
    title: await page.title(),
  };
}
`;
}

async function ownerRequest(origin, token, path, init = {}) {
  return await fetchJson(`${origin}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(init.body === undefined ? {} : { 'Content-Type': 'application/json' }),
      ...init.headers,
    },
  });
}

async function machineRequest(origin, token, path, init, expectedStatuses) {
  const response = await fetch(`${origin}${path}`, {
    ...init,
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${token}`,
      ...(init?.body === undefined ? {} : { 'Content-Type': 'application/json' }),
      ...init?.headers,
    },
    signal: AbortSignal.timeout(30_000),
  });
  const text = await response.text();
  const body = text ? JSON.parse(text) : null;
  if (!expectedStatuses.includes(response.status)) {
    throw new Error(`machine request returned HTTP ${response.status} (${body?.code ?? 'unknown'})`);
  }
  return body;
}

async function obtainMachineToken(secret) {
  const response = await fetch(
    'http://localhost:8091/realms/browserpane-dev/protocol/openid-connect/token',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        grant_type: 'client_credentials',
        client_id: MACHINE_CLIENT_ID,
        client_secret: secret,
      }),
      signal: AbortSignal.timeout(30_000),
    },
  );
  const body = await response.json();
  if (!response.ok || typeof body.access_token !== 'string') {
    throw new Error('fake BPM client-credentials token acquisition failed');
  }
  return body.access_token;
}

async function localMachineSecret() {
  const realm = JSON.parse(
    await fs.readFile(new URL('../../../../deploy/keycloak/browserpane-dev-realm.json', import.meta.url), 'utf8'),
  );
  const secret = realm.clients?.find((client) => client.clientId === MACHINE_CLIENT_ID)?.secret;
  if (typeof secret !== 'string' || !secret) {
    throw new Error('the local Keycloak fixture does not contain the fake BPM caller');
  }
  return secret;
}

function tokenIssuer(token) {
  const payload = token.split('.')[1];
  if (!payload) throw new Error('owner access token is malformed');
  const decoded = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8'));
  if (typeof decoded.iss !== 'string' || !decoded.iss) throw new Error('owner token omitted issuer');
  return decoded.iss;
}

async function createProject(origin, ownerToken) {
  return await ownerRequest(origin, ownerToken, '/api/v1/projects', {
    method: 'POST',
    body: JSON.stringify({
      name: `Workflow endpoint smoke ${Date.now()}`,
      description: 'Temporary real-Compose fake BPM conformance project.',
      labels: { suite: 'workflow-endpoint-compose-smoke' },
      quotas: {
        max_active_sessions: 4,
        max_active_workflow_runs: 4,
        max_retained_storage_bytes: 1_073_741_824,
      },
    }),
  });
}

async function createWorkflow(origin, ownerToken) {
  return await ownerRequest(origin, ownerToken, '/api/v1/workflows', {
    method: 'POST',
    body: JSON.stringify({
      name: `workflow-endpoint-smoke-${Date.now()}`,
      description: 'Deterministic browser workflow for fake BPM polling.',
      labels: { suite: 'workflow-endpoint-compose-smoke' },
    }),
  });
}

async function createWorkflowVersion(origin, ownerToken, workflowId, source) {
  return await ownerRequest(origin, ownerToken, `/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/endpoint/run.ts',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      input_schema: endpointInputSchema(),
      output_schema: endpointOutputSchema(),
      package: {
        package_id: 'browserpane.workflow-endpoint-smoke.v1',
        format_version: 'browserpane.workflow-package/v1',
        runtime: {
          language: 'typescript',
          browserpane_api_version: 'v1',
          node_major_version: 22,
          playwright_major_version: 1,
          playwright_minor_version: 59,
        },
        requirements: {
          default_session: {
            project_id: null,
            browser_context: { mode: 'fresh', context_id: null },
            network_identity: { egress_profile_id: null },
            capabilities: {
              browser_input: true,
              clipboard: false,
              audio: false,
              microphone: false,
              camera: false,
              file_transfer: false,
              resize: true,
            },
            recording: { mode: 'manual', format: 'webm', retention_sec: null },
            extension_ids: [],
            labels: { origin: 'workflow-endpoint-compose-smoke' },
          },
          allowed_credential_binding_ids: [],
          allowed_extension_ids: [],
          allowed_file_workspace_ids: [],
        },
        execution: {
          timeout_ms: 120_000,
          assertions: ['schema-valid-result'],
          safe_cancellation_points: ['before-navigation'],
          side_effect_checkpoints: ['after-navigation'],
        },
        publication: {
          reviewer: 'browserpane-workflow-endpoint-smoke',
          reviewed_at: '2026-08-21T08:00:00Z',
          decision: 'approved',
          fresh_context_replay: true,
          scenarios: [
            { kind: 'happy_path', result: 'passed' },
            { kind: 'validation', result: 'passed' },
            { kind: 'missing_element', result: 'not_applicable' },
            { kind: 'authentication_challenge', result: 'passed' },
            { kind: 'portal_failure', result: 'passed' },
            { kind: 'runtime_failure', result: 'passed' },
            { kind: 'cancellation', result: 'passed' },
            { kind: 'ambiguous_post_side_effect', result: 'passed' },
          ],
        },
      },
    }),
  });
}

function endpointInputSchema() {
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    type: 'object',
    additionalProperties: false,
    required: ['reporting_period', 'target_url'],
    properties: {
      reporting_period: { type: 'string' },
      target_url: { type: 'string' },
      delay_ms: { type: 'integer', minimum: 0, maximum: 30_000 },
      invalid_output: { type: 'boolean' },
    },
  };
}

function endpointOutputSchema() {
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    type: 'object',
    additionalProperties: false,
    required: ['status', 'reporting_period', 'title'],
    properties: {
      status: { const: 'complete' },
      reporting_period: { type: 'string' },
      title: { type: 'string' },
    },
  };
}

function endpointRequest(workflow, version, timeout = 30) {
  return {
    endpoint_key: ENDPOINT_KEY,
    purpose: 'Retrieve one supplier report for an external process.',
    workflow_definition_id: workflow.id,
    workflow_definition_version_id: version.id,
    workflow_version: version.version,
    input_schema: endpointInputSchema(),
    output_schema: endpointOutputSchema(),
    execution_timeout_seconds: timeout,
    inline_result_max_bytes: 65_536,
    artifact_behavior: { mode: 'authorized_references', retention_seconds: 86_400 },
    labels: { suite: 'workflow-endpoint-compose-smoke' },
  };
}

async function ensureServicePrincipal(origin, ownerToken, issuer, projectId) {
  const existing = (await ownerRequest(origin, ownerToken, '/api/v1/service-principals'))
    .service_principals
    ?.find((candidate) => candidate.client_id === MACHINE_CLIENT_ID && candidate.issuer === issuer);
  if (existing) {
    return await updateServicePrincipal(origin, ownerToken, existing, {
      scopes: ALL_SCOPES,
      allowed_project_ids: [projectId],
      state: 'active',
    });
  }
  return await ownerRequest(origin, ownerToken, '/api/v1/service-principals', {
    method: 'POST',
    body: JSON.stringify({
      name: 'Fake BPM conformance caller',
      description: 'Local client-credentials caller for endpoint smoke validation.',
      client_id: MACHINE_CLIENT_ID,
      issuer,
      scopes: ALL_SCOPES,
      allowed_project_ids: [projectId],
      state: 'active',
      labels: { suite: 'workflow-endpoint-compose-smoke' },
    }),
  });
}

async function updateServicePrincipal(origin, ownerToken, principal, overrides) {
  return await ownerRequest(origin, ownerToken, `/api/v1/service-principals/${principal.id}`, {
    method: 'PUT',
    body: JSON.stringify({
      name: principal.name,
      description: principal.description,
      client_id: principal.client_id,
      issuer: principal.issuer,
      scopes: principal.scopes,
      allowed_project_ids: principal.allowed_project_ids,
      state: principal.state,
      labels: principal.labels,
      ...overrides,
    }),
  });
}

async function invoke(origin, machineToken, projectId, input, suffix) {
  return await machineRequest(
    origin,
    machineToken,
    `/api/v1/projects/${projectId}/workflow-endpoints/${ENDPOINT_KEY}/invocations`,
    {
      method: 'POST',
      headers: { 'Idempotency-Key': `workflow-endpoint-smoke-${suffix}-${Date.now()}` },
      body: JSON.stringify({ input, source_system: 'fake-bpm', source_reference: suffix }),
    },
    [202],
  );
}

async function pollMachine(origin, machineToken, invocation, expectedCategory, timeoutMs) {
  return await poll(
    `endpoint invocation ${expectedCategory}`,
    () => machineRequest(origin, machineToken, invocation.links.self_path, {}, [200]),
    (candidate) => candidate?.outcome?.category === expectedCategory,
    timeoutMs,
    250,
  );
}

async function ownerRun(origin, ownerToken, runId) {
  return await ownerRequest(origin, ownerToken, `/api/v1/workflow-runs/${runId}`);
}

async function transitionEndpointRun(origin, ownerToken, invocation, state, error, data) {
  const run = await poll(
    'endpoint workflow run executor readiness',
    () => ownerRun(origin, ownerToken, invocation.run_id),
    (candidate) => Boolean(candidate?.session_id) && ['starting', 'running'].includes(candidate.state),
    30_000,
    250,
  );
  const access = await ownerRequest(origin, ownerToken, `/api/v1/sessions/${run.session_id}/automation-access`, { method: 'POST' });
  await fetchJson(`${origin}/api/v1/workflow-runs/${run.id}/state`, {
    method: 'POST',
    headers: { 'x-bpane-automation-access-token': access.token, 'Content-Type': 'application/json' },
    body: JSON.stringify({ state, error, message: `fake BPM ${state}`, data }),
  });
  return run.session_id;
}

async function assertAuthorizationDenials(origin, ownerToken, machineToken, projectId, principal) {
  const input = { reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL };
  const path = `/api/v1/projects/${projectId}/workflow-endpoints/${ENDPOINT_KEY}/invocations`;
  const attempt = async (suffix) => machineRequest(origin, machineToken, path, {
    method: 'POST',
    headers: { 'Idempotency-Key': `denied-${suffix}-${Date.now()}` },
    body: JSON.stringify({ input }),
  }, [403]);
  principal = await updateServicePrincipal(origin, ownerToken, principal, { scopes: ['workflow-endpoints:read'] });
  if ((await attempt('scope'))?.code !== 'workflow_endpoint_authorization_denied') throw new Error('insufficient scope was not denied');
  principal = await updateServicePrincipal(origin, ownerToken, principal, { scopes: ALL_SCOPES, allowed_project_ids: [] });
  if ((await attempt('project'))?.code !== 'workflow_endpoint_authorization_denied') throw new Error('cross-project caller was not denied');
  principal = await updateServicePrincipal(origin, ownerToken, principal, { allowed_project_ids: [projectId], state: 'disabled' });
  if ((await attempt('disabled'))?.code !== 'workflow_endpoint_authorization_denied') throw new Error('disabled caller was not denied');
  return await updateServicePrincipal(origin, ownerToken, principal, { state: 'active' });
}

async function main() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-workflow-endpoint-compose-smoke.mjs');
  const browser = await launchChrome(chromium, options);
  let context;
  let source;
  let ownerToken = '';
  let project;
  let principal;
  const sessionIds = new Set();
  try {
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } });
    const page = await context.newPage();
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    ownerToken = (await getAccessToken(page)) ?? '';
    if (!ownerToken) throw new Error('owner login did not yield an access token');
    const origin = apiOrigin(options);
    await cleanupWorkflowSmokeSessions(ownerToken, options, log);
    source = await createLocalWorkflowRepo('.workflow-endpoint-smoke-repo-', {
      'workflows/endpoint/run.ts': workflowEntrypoint(),
    });
    log('Building the workflow worker used by the real endpoint smoke');
    buildWorkflowWorkerImage();
    project = await createProject(origin, ownerToken);
    const workflow = await createWorkflow(origin, ownerToken);
    const version = await createWorkflowVersion(origin, ownerToken, workflow.id, source);
    principal = await ensureServicePrincipal(origin, ownerToken, tokenIssuer(ownerToken), project.id);
    const endpoint = await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints`, {
      method: 'POST',
      body: JSON.stringify(endpointRequest(workflow, version)),
    });
    await ownerRequest(origin, ownerToken, `${endpoint.grants_path}`, {
      method: 'POST',
      body: JSON.stringify({ service_principal_id: principal.id, operations: ALL_OPERATIONS }),
    });
    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}/activate`, { method: 'POST' });
    const machineSecret = await localMachineSecret();
    const machineToken = await obtainMachineToken(machineSecret);

    const conformance = await runWorkflowEndpointConformance({
      BPANE_ACCESS_TOKEN: machineToken,
      BPANE_BASE_URL: origin,
      BPANE_BPM_PROJECT_ID: project.id,
      BPANE_BPM_ENDPOINT_KEY: ENDPOINT_KEY,
      BPANE_BPM_INPUT_JSON: JSON.stringify({ reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL }),
      BPANE_BPM_CHANGED_INPUT_JSON: JSON.stringify({ reporting_period: '2026-Q4', target_url: RUNTIME_TARGET_URL }),
      BPANE_BPM_INVALID_INPUT_JSON: JSON.stringify({ reporting_period: 20263 }),
      BPANE_BPM_POLL_TIMEOUT_MS: String(Math.max(options.connectTimeoutMs, 120_000)),
    });
    const successRun = await ownerRun(origin, ownerToken, conformance.run_id);
    if (successRun.session_id) {
      sessionIds.add(successRun.session_id);
      await deleteSession(ownerToken, options, successRun.session_id);
      sessionIds.delete(successRun.session_id);
    }

    const invalidOutput = await invoke(origin, machineToken, project.id, {
      reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL, invalid_output: true,
    }, 'invalid-output');
    const invalidOutputResult = await pollMachine(origin, machineToken, invalidOutput, 'validation_failure', 120_000);
    if (invalidOutputResult.state === 'succeeded') throw new Error('schema-invalid output reached succeeded');
    const invalidOutputRun = await ownerRun(origin, ownerToken, invalidOutput.run_id);
    if (invalidOutputRun.session_id) await deleteSession(ownerToken, options, invalidOutputRun.session_id);

    const cancellation = await invoke(origin, machineToken, project.id, {
      reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL, delay_ms: 15_000,
    }, 'cancel');
    await machineRequest(origin, machineToken, cancellation.links.cancel_path, { method: 'POST' }, [200]);
    await pollMachine(origin, machineToken, cancellation, 'cancellation', 30_000);
    const cancelledRun = await ownerRun(origin, ownerToken, cancellation.run_id);
    if (cancelledRun.session_id) await deleteSession(ownerToken, options, cancelledRun.session_id);

    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}/disable`, { method: 'POST' });
    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}`, {
      method: 'PUT', body: JSON.stringify(endpointRequest(workflow, version, 1)),
    });
    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}/activate`, { method: 'POST' });
    const timed = await invoke(origin, machineToken, project.id, {
      reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL, delay_ms: 5_000,
    }, 'timeout');
    await pollMachine(origin, machineToken, timed, 'timeout', 30_000);
    const timedRun = await ownerRun(origin, ownerToken, timed.run_id);
    if (timedRun.session_id) await deleteSession(ownerToken, options, timedRun.session_id);

    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}/disable`, { method: 'POST' });
    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}`, {
      method: 'PUT', body: JSON.stringify(endpointRequest(workflow, version, 30)),
    });
    await ownerRequest(origin, ownerToken, `/api/v1/projects/${project.id}/workflow-endpoints/${ENDPOINT_KEY}/activate`, { method: 'POST' });

    const challenge = await invoke(origin, machineToken, project.id, {
      reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL, delay_ms: 15_000,
    }, 'challenge');
    sessionIds.add(await transitionEndpointRun(origin, ownerToken, challenge, 'awaiting_input', null, {
      intervention_request: { request_id: 'challenge-1', kind: 'authentication', prompt: 'MFA required' },
    }));
    await pollMachine(origin, machineToken, challenge, 'external_intervention_required', 30_000);
    const challengeRun = await ownerRun(origin, ownerToken, challenge.run_id);
    if (challengeRun.session_id) {
      await deleteSession(ownerToken, options, challengeRun.session_id);
      sessionIds.delete(challengeRun.session_id);
    }

    const terminalCases = [
      ['policy', 'policy_denied:local-smoke', 'policy_denial', 'none'],
      ['retryable', 'retryable_technical_failure:local-smoke', 'retryable_technical_failure', 'none'],
      ['permanent', 'local-smoke-permanent-failure', 'permanent_technical_failure', 'none'],
      ['uncertain', 'business_failure:ambiguous-submit', 'business_failure', 'uncertain'],
    ];
    for (const [name, error, category, sideEffectState] of terminalCases) {
      const invocation = await invoke(origin, machineToken, project.id, {
        reporting_period: '2026-Q3', target_url: RUNTIME_TARGET_URL, delay_ms: 15_000,
      }, name);
      sessionIds.add(await transitionEndpointRun(origin, ownerToken, invocation, 'failed', error, {
        workflow_endpoint: { side_effect_state: sideEffectState },
      }));
      const terminal = await pollMachine(origin, machineToken, invocation, category, 30_000);
      if (sideEffectState === 'uncertain' && terminal.outcome.retryable !== false) {
        throw new Error('uncertain browser side effects were presented as retryable');
      }
      const terminalRun = await ownerRun(origin, ownerToken, invocation.run_id);
      if (terminalRun.session_id) {
        await deleteSession(ownerToken, options, terminalRun.session_id);
        sessionIds.delete(terminalRun.session_id);
      }
    }

    principal = await assertAuthorizationDenials(origin, ownerToken, machineToken, project.id, principal);
    console.log(JSON.stringify({
      ok: true,
      project_id: project.id,
      endpoint_key: ENDPOINT_KEY,
      success_run_id: conformance.run_id,
      output_validation: 'passed',
      cancellation: 'passed',
      timeout: 'passed',
      external_intervention: 'passed',
      typed_failures: 'passed',
      uncertain_side_effect: 'passed',
      authorization_denials: 'passed',
    }, null, 2));
  } finally {
    if (ownerToken) {
      for (const sessionId of sessionIds) await deleteSession(ownerToken, options, sessionId).catch(() => {});
      if (principal) await updateServicePrincipal(apiOrigin(options), ownerToken, principal, { state: 'disabled' }).catch(() => {});
    }
    await source?.cleanup?.().catch(() => {});
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(`[workflow-endpoint-compose-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`);
  process.exitCode = 1;
});
