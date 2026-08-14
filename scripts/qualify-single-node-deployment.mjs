#!/usr/bin/env node

import { execFileSync, spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const API_ORIGIN = 'http://localhost:18080';
const TOKEN_URL = 'http://localhost:18091/realms/browserpane-dev/protocol/openid-connect/token';
const SOURCE_REPOSITORY = 'https://github.com/ITmedes/browserpane';
const COMPOSE_ARGS = [
  'compose',
  '--project-name',
  'bpane-single-node-fixture',
  '--env-file',
  'deploy/single-node/generated/fixture.env',
  '-f',
  'deploy/single-node/compose.yml',
  '-f',
  'deploy/single-node/fixture/compose.yml',
];
const TIMEOUT_MS = 180_000;

async function main() {
  assertFixtureReady();
  const source = resolveWorkflowSource();
  let accessToken = await issueServiceToken();
  const sessionIds = [];

  try {
    const workspace = await api('/api/v1/file-workspaces', accessToken, {
      method: 'POST',
      body: {
        name: `single-node-qualification-${Date.now()}`,
        description: 'Single-node deployment produced-file qualification',
        labels: { suite: 'single-node-qualification' },
      },
    });
    const workflow = await api('/api/v1/workflows', accessToken, {
      method: 'POST',
      body: {
        name: `Single-node qualification ${Date.now()}`,
        description: 'Broker-launched workflow and persistence qualification',
        labels: { suite: 'single-node-qualification' },
      },
    });
    const version = await api(`/api/v1/workflows/${workflow.id}/versions`, accessToken, {
      method: 'POST',
      body: {
        version: 'v1',
        executor: 'playwright',
        entrypoint: 'dev/workflows/single-node-qualification/run.mjs',
        source: {
          kind: 'git',
          repository_url: SOURCE_REPOSITORY,
          ref: source.ref,
          root_path: 'dev',
        },
        input_schema: {
          type: 'object',
          required: ['target_url', 'output_workspace_id'],
          properties: {
            target_url: { type: 'string' },
            output_workspace_id: { type: 'string' },
          },
        },
        output_schema: {
          type: 'object',
          required: ['body', 'final_url', 'output_file_id'],
        },
        default_session: {
          labels: { origin: 'single-node-qualification' },
          recording: { mode: 'manual', format: 'webm' },
        },
        allowed_file_workspace_ids: [workspace.id],
      },
    });
    assert(version.source?.resolved_commit === source.commit,
      `workflow source resolved ${version.source?.resolved_commit ?? 'no commit'}, expected ${source.commit}`);

    const createdRun = await api('/api/v1/workflow-runs', accessToken, {
      method: 'POST',
      body: {
        workflow_id: workflow.id,
        version: 'v1',
        input: {
          target_url: 'http://web:8080/healthz',
          output_workspace_id: workspace.id,
        },
        labels: { suite: 'single-node-qualification' },
      },
    });
    const sessionId = createdRun.session_id;
    assert(sessionId, 'workflow run did not create a session');
    sessionIds.push(sessionId);

    const succeededRun = await poll('workflow success', async () => {
      return await api(`/api/v1/workflow-runs/${createdRun.id}`, accessToken);
    }, (run) => run.state === 'succeeded' || isTerminalFailure(run.state));
    assert(succeededRun.state === 'succeeded',
      `workflow run finished in ${succeededRun.state}: ${succeededRun.error ?? 'no error'}`);
    assert(succeededRun.output?.body === 'ready', 'workflow did not reach the internal web health endpoint');
    assert(succeededRun.output?.final_url === 'http://web:8080/healthz',
      `workflow returned unexpected URL ${succeededRun.output?.final_url}`);
    assert(succeededRun.produced_files?.length === 1, 'workflow did not produce exactly one file');

    const producedFile = succeededRun.produced_files[0];
    const beforeRestart = await download(producedFile.content_path, accessToken);
    assert(beforeRestart.toString('utf8').includes('body=ready'),
      'produced file does not contain workflow evidence');

    const secondRun = await api('/api/v1/workflow-runs', accessToken, {
      method: 'POST',
      body: {
        workflow_id: workflow.id,
        version: 'v1',
        input: {
          target_url: 'http://web:8080/healthz',
          output_workspace_id: workspace.id,
        },
        labels: { suite: 'single-node-qualification', ordinal: 'secondary' },
      },
    });
    const secondSessionId = secondRun.session_id;
    assert(secondSessionId && secondSessionId !== sessionId,
      'second workflow run did not create an independent session');
    sessionIds.push(secondSessionId);
    const secondSucceededRun = await poll('second workflow success', async () => {
      return await api(`/api/v1/workflow-runs/${secondRun.id}`, accessToken);
    }, (run) => run.state === 'succeeded' || isTerminalFailure(run.state));
    assert(secondSucceededRun.state === 'succeeded',
      `second workflow run finished in ${secondSucceededRun.state}: ${secondSucceededRun.error ?? 'no error'}`);
    assert(secondSucceededRun.output?.body === 'ready',
      'second workflow did not reach the internal web health endpoint');

    const [runtimeBeforeRestart] = await waitForRuntimeCount(sessionId, 1);
    const [secondRuntimeBeforeRestart] = await waitForRuntimeCount(secondSessionId, 1);
    assert(runtimeBeforeRestart !== secondRuntimeBeforeRestart,
      'independent workflow sessions shared one runtime container');

    restartControlPlane();
    await waitForHttp(`${API_ORIGIN}/healthz`);
    accessToken = await issueServiceToken();
    const retainedRun = await api(`/api/v1/workflow-runs/${createdRun.id}`, accessToken);
    assert(retainedRun.state === 'succeeded', 'workflow run was not retained across restart');
    const afterRestart = await download(producedFile.content_path, accessToken);
    assert(beforeRestart.equals(afterRestart), 'produced file changed across restart');
    const [runtimeAfterRestart] = await waitForRuntimeCount(sessionId, 1);
    const [secondRuntimeAfterRestart] = await waitForRuntimeCount(secondSessionId, 1);
    assert(runtimeAfterRestart === runtimeBeforeRestart,
      'primary session runtime changed across control-plane restart');
    assert(secondRuntimeAfterRestart === secondRuntimeBeforeRestart,
      'secondary session runtime changed across control-plane restart');

    assertGatewayDockerDenied();
    assertSensitiveMarkersAbsent();

    console.log(JSON.stringify({
      workflowId: workflow.id,
      workflowVersion: version.version,
      sourceCommit: source.commit,
      runId: createdRun.id,
      sessionId,
      secondRunId: secondRun.id,
      secondSessionId,
      producedFileId: producedFile.file_id,
      producedFileBytes: afterRestart.length,
      retainedAcrossRestart: true,
      distinctRuntimeContainers: true,
      runtimeCountAfterRestart: sessionIds.length,
      gatewayDockerDenied: true,
      sensitiveMarkerScan: true,
    }, null, 2));
  } finally {
    if (accessToken) {
      for (const sessionId of sessionIds) await removeSession(accessToken, sessionId);
    }
  }
}

function resolveWorkflowSource() {
  const branch = git(['branch', '--show-current'], ROOT).trim();
  assert(branch && branch !== 'HEAD', 'single-node qualification requires a named branch');
  const commit = git(['rev-parse', 'HEAD'], ROOT).trim();
  const ref = `refs/heads/${branch}`;
  const remote = git(['ls-remote', '--heads', 'origin', ref], ROOT).trim();
  assert(remote.startsWith(`${commit}\t`),
    `origin ${ref} must point to local commit ${commit}; commit and push the qualification fixture first`);
  return { commit, ref };
}

async function issueServiceToken() {
  const response = await fetch(TOKEN_URL, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'client_credentials',
      client_id: 'bpane-mcp-bridge',
      client_secret: 'bpane-mcp-bridge-secret',
    }),
  });
  assert(response.ok, `fixture token request failed with HTTP ${response.status}`);
  const payload = await response.json();
  assert(payload.access_token, 'fixture token response is missing access_token');
  return payload.access_token;
}

async function api(resourcePath, accessToken, options = {}) {
  const headers = { Authorization: `Bearer ${accessToken}` };
  const init = { method: options.method ?? 'GET', headers };
  if (options.body !== undefined) {
    headers['content-type'] = 'application/json';
    init.body = JSON.stringify(options.body);
  }
  const response = await fetch(`${API_ORIGIN}${resourcePath}`, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`${init.method} ${resourcePath} failed with HTTP ${response.status}${detail ? `: ${detail}` : ''}`);
  }
  return await response.json();
}

async function download(resourcePath, accessToken) {
  const response = await fetch(`${API_ORIGIN}${resourcePath}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  assert(response.ok, `download ${resourcePath} failed with HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}

async function removeSession(accessToken, sessionId) {
  let response = await fetch(`${API_ORIGIN}/api/v1/sessions/${sessionId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  }).catch(() => null);
  if (response?.status === 409) {
    response = await fetch(`${API_ORIGIN}/api/v1/sessions/${sessionId}/kill`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${accessToken}` },
    }).catch(() => null);
  }
  if (response && !response.ok && response.status !== 404) {
    console.warn(`session cleanup returned HTTP ${response.status}`);
  }
}

function restartControlPlane() {
  for (const service of ['runtime-broker', 'gateway', 'web']) {
    dockerCompose(['restart', service]);
    waitForContainerHealth(`bpane-single-node-fixture-${service}-1`);
  }
}

function waitForContainerHealth(containerName) {
  const started = Date.now();
  while (Date.now() - started < TIMEOUT_MS) {
    const result = spawnSync('docker', [
      'inspect',
      '--format',
      '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}',
      containerName,
    ], { encoding: 'utf8' });
    if (result.status === 0 && result.stdout.trim() === 'healthy') return;
    sleepSync(500);
  }
  throw new Error(`${containerName} did not become healthy after restart`);
}

async function waitForHttp(url) {
  await poll('web readiness', async () => {
    try {
      const response = await fetch(url);
      return response.ok;
    } catch {
      return false;
    }
  }, Boolean);
}

async function waitForRuntimeCount(sessionId, expected) {
  return await poll(`runtime count ${expected} for ${sessionId}`, () => {
    const result = spawnSync('docker', [
      'ps',
      '--filter',
      `label=browserpane.runtime.resource_id=${sessionId}`,
      '--format',
      '{{.Names}}',
    ], { encoding: 'utf8' });
    assert(result.status === 0, 'docker runtime inspection failed');
    return result.stdout.trim() ? result.stdout.trim().split('\n') : [];
  }, (containers) => containers.length === expected);
}

function assertGatewayDockerDenied() {
  const inspection = dockerInspect('bpane-single-node-fixture-gateway-1');
  const mounts = inspection.Mounts ?? [];
  assert(!mounts.some((mount) => mount.Source?.includes('docker.sock')),
    'gateway unexpectedly mounts the Docker socket');
  assert(!Object.hasOwn(inspection.NetworkSettings?.Networks ?? {}, 'bpane-single-node-fixture-docker-control'),
    'gateway unexpectedly joins docker-control');
  const probe = spawnSync('docker', [
    'exec',
    'bpane-single-node-fixture-gateway-1',
    'curl',
    '-fsS',
    '--connect-timeout',
    '2',
    'http://docker-proxy:2375/_ping',
  ], { encoding: 'utf8' });
  assert(probe.status !== 0, 'gateway unexpectedly reached the Docker proxy');
}

function assertSensitiveMarkersAbsent() {
  const markers = [
    'single-node-fixture-vault-token',
    'bpane-runtime-broker-gateway-secret',
    'bpane-mcp-bridge-secret',
    'postgres://browserpane:single-node-fixture',
  ];
  const evidence = ['gateway', 'runtime-broker', 'web', 'docker-proxy'].map((service) => {
    const container = `bpane-single-node-fixture-${service}-1`;
    const inspect = execFileSync('docker', ['inspect', container], { encoding: 'utf8' });
    const logs = spawnSync('docker', ['logs', container], { encoding: 'utf8' });
    return `${inspect}\n${logs.stdout}\n${logs.stderr}`;
  }).join('\n');
  for (const marker of markers) assert(!evidence.includes(marker), `sensitive marker leaked: ${marker}`);
}

function assertFixtureReady() {
  const result = spawnSync('curl', ['-fsS', `${API_ORIGIN}/healthz`], { encoding: 'utf8' });
  assert(result.status === 0, 'single-node fixture is not ready');
}

function dockerCompose(args) {
  const result = spawnSync('docker', [...COMPOSE_ARGS, ...args], {
    cwd: ROOT,
    encoding: 'utf8',
    stdio: 'inherit',
  });
  assert(result.status === 0, `docker compose ${args.join(' ')} failed`);
}

function dockerInspect(containerName) {
  return JSON.parse(execFileSync('docker', ['inspect', containerName], { encoding: 'utf8' }))[0];
}

function git(args, directory) {
  return execFileSync('git', args, { cwd: directory, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}

async function poll(description, operation, predicate, timeoutMs = TIMEOUT_MS) {
  const started = Date.now();
  let value;
  while (Date.now() - started < timeoutMs) {
    value = await operation();
    if (predicate(value)) return value;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`timed out waiting for ${description}: ${JSON.stringify(value)}`);
}

function sleepSync(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function isTerminalFailure(state) {
  return ['failed', 'cancelled', 'timed_out'].includes(state);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

main().catch((error) => {
  console.error(`[single-node-qualification] ${error instanceof Error ? error.stack ?? error.message : String(error)}`);
  process.exitCode = 1;
});
