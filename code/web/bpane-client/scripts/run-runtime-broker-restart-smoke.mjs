import { execFileSync } from 'node:child_process';
import process from 'node:process';

import { chromium } from 'playwright-core';

import {
  apiOrigin,
  configurePage,
  createLogger,
  ensureLoggedIn,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
} from './workflow-smoke-lib.mjs';

const log = createLogger('runtime-broker-restart-smoke');
const brokerContainer = process.env.BPANE_RUNTIME_BROKER_CONTAINER ?? 'deploy-runtime-broker-1';

function runtimeContainerName(sessionId) {
  return `bpane-runtime-${sessionId.replaceAll('-', '')}`;
}

function inspectRuntimeContainer(sessionId) {
  return execFileSync(
    'docker',
    ['ps', '-q', '--filter', `name=^/${runtimeContainerName(sessionId)}$`],
    { encoding: 'utf8' },
  ).trim();
}

function brokerIsReady() {
  try {
    execFileSync(
      'docker',
      [
        'exec',
        brokerContainer,
        'curl',
        '-fsS',
        '--connect-timeout',
        '2',
        '--max-time',
        '3',
        'http://localhost:8940/readyz',
      ],
      { stdio: 'ignore' },
    );
    return true;
  } catch {
    return false;
  }
}

async function ownerRequest(options, accessToken, path, init = {}) {
  return await fetchJson(`${apiOrigin(options)}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${accessToken}`,
      ...(init.body ? { 'Content-Type': 'application/json' } : {}),
      ...init.headers,
    },
  });
}

async function main() {
  const options = parseSmokeArgs(
    process.argv.slice(2),
    'run-runtime-broker-restart-smoke.mjs',
  );
  const browser = await launchChrome(chromium, options);
  let context = null;
  let sessionId = '';
  let accessToken = '';

  try {
    context = await browser.newContext({ viewport: { width: 1440, height: 960 } });
    const page = await context.newPage();
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    accessToken = (await getAccessToken(page)) ?? '';
    if (!accessToken) {
      throw new Error('Runtime broker restart smoke failed to acquire an access token.');
    }

    const session = await ownerRequest(options, accessToken, '/api/v1/sessions', {
      method: 'POST',
      body: JSON.stringify({
        idle_timeout_sec: 300,
        recording: { mode: 'manual', format: 'webm' },
        labels: { suite: 'runtime-broker-restart-smoke' },
      }),
    });
    sessionId = session.id;
    log(`Starting broker-backed runtime for session ${sessionId}`);
    await ownerRequest(
      options,
      accessToken,
      `/api/v1/sessions/${sessionId}/automation-access`,
      { method: 'POST' },
    );
    const initialContainerId = await poll(
      'initial broker-backed runtime container',
      async () => inspectRuntimeContainer(sessionId),
      Boolean,
      options.connectTimeoutMs,
      500,
    );

    log('Restarting runtime broker while the browser runtime remains live');
    execFileSync('docker', ['restart', brokerContainer], { stdio: 'inherit' });
    await poll(
      'runtime broker readiness after restart',
      async () => brokerIsReady(),
      Boolean,
      options.connectTimeoutMs,
      500,
    );

    const liveContainerId = inspectRuntimeContainer(sessionId);
    if (liveContainerId !== initialContainerId) {
      throw new Error('Runtime broker restart replaced or lost the live browser container.');
    }
    await ownerRequest(options, accessToken, `/api/v1/sessions/${sessionId}/status`);
    await ownerRequest(
      options,
      accessToken,
      `/api/v1/sessions/${sessionId}/automation-access`,
      { method: 'POST' },
    );
    if (inspectRuntimeContainer(sessionId) !== initialContainerId) {
      throw new Error('Post-restart access launched a duplicate browser container.');
    }

    await ownerRequest(options, accessToken, `/api/v1/sessions/${sessionId}/stop`, {
      method: 'POST',
    });
    await poll(
      'browser runtime cleanup after broker restart',
      async () => inspectRuntimeContainer(sessionId),
      (containerId) => containerId === '',
      options.connectTimeoutMs,
      500,
    );

    console.log(JSON.stringify({
      sessionId,
      preservedContainerId: initialContainerId,
      brokerRestarted: true,
      duplicateRuntime: false,
      stoppedAfterRestart: true,
    }, null, 2));
  } finally {
    if (sessionId && accessToken) {
      await fetch(`${apiOrigin(options)}/api/v1/sessions/${sessionId}/kill`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${accessToken}` },
      }).catch(() => {});
    }
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[runtime-broker-restart-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
