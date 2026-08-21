#!/usr/bin/env node

import crypto from 'node:crypto';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const TERMINAL_STATES = new Set(['succeeded', 'failed', 'cancelled', 'timed_out']);

function required(env, name) {
  const value = env[name]?.trim();
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function parseJson(value, fallback, name) {
  if (!value) return fallback;
  try {
    return JSON.parse(value);
  } catch {
    throw new Error(`${name} must contain valid JSON`);
  }
}

async function readResponse(response) {
  const text = await response.text();
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return { message: 'response was not JSON' };
  }
}

async function requestJson(fetchImpl, url, token, init = {}, expectedStatuses = [200]) {
  const response = await fetchImpl(url, {
    ...init,
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${token}`,
      ...(init.body === undefined ? {} : { 'Content-Type': 'application/json' }),
      ...init.headers,
    },
    signal: AbortSignal.timeout(30_000),
  });
  const body = await readResponse(response);
  if (!expectedStatuses.includes(response.status)) {
    const code = body?.code ?? `http_${response.status}`;
    throw new Error(`workflow endpoint request failed with ${code}`);
  }
  return { status: response.status, body };
}

async function obtainToken(env, fetchImpl) {
  if (env.BPANE_ACCESS_TOKEN?.trim()) {
    return env.BPANE_ACCESS_TOKEN.trim();
  }
  const tokenUrl = required(env, 'BPANE_BPM_OIDC_TOKEN_URL');
  const clientId = required(env, 'BPANE_BPM_CLIENT_ID');
  const clientSecret = required(env, 'BPANE_BPM_CLIENT_SECRET');
  const form = new URLSearchParams({
    grant_type: 'client_credentials',
    client_id: clientId,
    client_secret: clientSecret,
  });
  if (env.BPANE_BPM_OIDC_SCOPES?.trim()) {
    form.set('scope', env.BPANE_BPM_OIDC_SCOPES.trim());
  }
  const response = await fetchImpl(tokenUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: form.toString(),
    signal: AbortSignal.timeout(30_000),
  });
  const body = await readResponse(response);
  if (!response.ok || typeof body?.access_token !== 'string' || !body.access_token) {
    throw new Error('client-credentials token acquisition failed');
  }
  return body.access_token;
}

export async function runWorkflowEndpointConformance(
  env = process.env,
  { fetchImpl = globalThis.fetch, stdout = process.stdout, wait = setTimeout } = {},
) {
  if (typeof fetchImpl !== 'function') {
    throw new Error('a fetch implementation is required');
  }
  const baseUrl = (env.BPANE_BASE_URL ?? 'http://localhost:8080').replace(/\/$/u, '');
  const projectId = required(env, 'BPANE_BPM_PROJECT_ID');
  const endpointKey = env.BPANE_BPM_ENDPOINT_KEY?.trim() || 'retrieve-supplier-report';
  const input = parseJson(
    env.BPANE_BPM_INPUT_JSON,
    { reporting_period: '2026-Q3' },
    'BPANE_BPM_INPUT_JSON',
  );
  const changedInput = parseJson(
    env.BPANE_BPM_CHANGED_INPUT_JSON,
    { ...input, reporting_period: '2026-Q4' },
    'BPANE_BPM_CHANGED_INPUT_JSON',
  );
  const invalidInput = parseJson(
    env.BPANE_BPM_INVALID_INPUT_JSON,
    { reporting_period: 20263 },
    'BPANE_BPM_INVALID_INPUT_JSON',
  );
  const sourceReference = env.BPANE_BPM_SOURCE_REFERENCE?.trim() || 'fake-bpm-process-001';
  const idempotencyKey = env.BPANE_BPM_IDEMPOTENCY_KEY?.trim()
    || `fake-bpm-${crypto.randomUUID()}`;
  const pollIntervalMs = Number.parseInt(env.BPANE_BPM_POLL_INTERVAL_MS ?? '250', 10);
  const pollTimeoutMs = Number.parseInt(env.BPANE_BPM_POLL_TIMEOUT_MS ?? '120000', 10);
  if (!Number.isInteger(pollIntervalMs) || pollIntervalMs < 1) {
    throw new Error('BPANE_BPM_POLL_INTERVAL_MS must be a positive integer');
  }
  if (!Number.isInteger(pollTimeoutMs) || pollTimeoutMs < pollIntervalMs) {
    throw new Error('BPANE_BPM_POLL_TIMEOUT_MS must be an integer at least as large as the poll interval');
  }
  const token = await obtainToken(env, fetchImpl);
  const invocationsUrl = `${baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/workflow-endpoints/${encodeURIComponent(endpointKey)}/invocations`;
  const invocationBody = {
    input,
    source_system: 'fake-bpm',
    source_reference: sourceReference,
  };
  const accepted = await requestJson(
    fetchImpl,
    invocationsUrl,
    token,
    {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
      body: JSON.stringify(invocationBody),
    },
    [200, 202],
  );
  if (!accepted.body?.id || !accepted.body?.links?.self_path || !accepted.body?.run_id) {
    throw new Error('accepted invocation omitted stable id, run, or polling link');
  }
  const replay = await requestJson(
    fetchImpl,
    invocationsUrl,
    token,
    {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
      body: JSON.stringify(invocationBody),
    },
    [200],
  );
  if (replay.body?.id !== accepted.body.id || replay.body?.run_id !== accepted.body.run_id) {
    throw new Error('identical replay did not return the original invocation and run');
  }
  const changed = await requestJson(
    fetchImpl,
    invocationsUrl,
    token,
    {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
      body: JSON.stringify({ ...invocationBody, input: changedInput }),
    },
    [409],
  );
  if (changed.body?.code !== 'idempotency_key_conflict') {
    throw new Error('changed idempotency replay did not return idempotency_key_conflict');
  }
  const invalid = await requestJson(
    fetchImpl,
    invocationsUrl,
    token,
    {
      method: 'POST',
      headers: { 'Idempotency-Key': `${idempotencyKey}-invalid` },
      body: JSON.stringify({ ...invocationBody, input: invalidInput }),
    },
    [422],
  );
  if (invalid.body?.code !== 'input_schema_validation_failed') {
    throw new Error('invalid input did not return input_schema_validation_failed');
  }

  const statusUrl = new URL(accepted.body.links.self_path, `${baseUrl}/`).toString();
  const deadline = Date.now() + pollTimeoutMs;
  let terminal = accepted.body;
  while (!TERMINAL_STATES.has(terminal.state) && Date.now() <= deadline) {
    await new Promise((resolve) => wait(resolve, pollIntervalMs));
    terminal = (await requestJson(fetchImpl, statusUrl, token, {}, [200])).body;
  }
  if (!TERMINAL_STATES.has(terminal?.state)) {
    throw new Error('workflow endpoint polling timed out before a terminal state');
  }
  const expectedState = env.BPANE_BPM_EXPECT_STATE?.trim() || 'succeeded';
  if (terminal.state !== expectedState) {
    throw new Error(`workflow endpoint reached ${terminal.state}, expected ${expectedState}`);
  }
  if (!terminal.outcome?.category || !terminal.side_effect_state) {
    throw new Error('terminal invocation omitted typed outcome or side-effect evidence');
  }
  if (terminal.side_effect_state === 'uncertain' && terminal.outcome.retryable !== false) {
    throw new Error('uncertain side effects were presented as retryable');
  }
  if (!Array.isArray(terminal.artifacts)
      || terminal.artifacts.some((artifact) => {
        return !artifact.file_id
          || !artifact.content_path
          || !artifact.sha256_hex
          || typeof artifact.byte_count !== 'number'
          || Object.hasOwn(artifact, 'content');
      })) {
    throw new Error('artifact references were incomplete or embedded binary content');
  }
  const summary = {
    ok: true,
    project_id: projectId,
    endpoint_key: endpointKey,
    invocation_id: terminal.id,
    run_id: terminal.run_id,
    state: terminal.state,
    outcome: terminal.outcome.category,
    side_effect_state: terminal.side_effect_state,
    artifact_count: terminal.artifacts.length,
    idempotent_replay_verified: true,
    changed_payload_conflict_verified: true,
    pre_runtime_validation_verified: true,
  };
  stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  return summary;
}

async function main() {
  try {
    await runWorkflowEndpointConformance();
  } catch (error) {
    process.stderr.write(`${JSON.stringify({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    })}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
