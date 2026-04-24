import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';
import { chromium } from 'playwright-core';

const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  certSpki: process.env.BPANE_BENCHMARK_CERT_SPKI ?? '',
  connectTimeoutMs: 30000,
  headless: false,
  outputPath: '',
};

const COMMON_CHROME_PATHS = [
  process.env.BPANE_BENCHMARK_CHROME,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
].filter(Boolean);

function parseArgs(argv) {
  const options = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === '--page-url' && next) {
      options.pageUrl = next;
      i++;
    } else if (arg === '--cert-spki' && next) {
      options.certSpki = next;
      i++;
    } else if (arg === '--connect-timeout-ms' && next) {
      options.connectTimeoutMs = Number(next);
      i++;
    } else if (arg === '--output' && next) {
      options.outputPath = next;
      i++;
    } else if (arg === '--headless') {
      options.headless = true;
    } else if (arg === '--help') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function printHelp() {
  console.log(`
Usage: node scripts/run-workflow-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[workflow-smoke] ${message}`);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function poll(description, fn, predicate, timeoutMs, intervalMs = 500) {
  const startedAt = Date.now();
  let lastValue = null;
  while (Date.now() - startedAt < timeoutMs) {
    lastValue = await fn();
    if (predicate(lastValue)) {
      return lastValue;
    }
    await sleep(intervalMs);
  }
  throw new Error(`Timed out waiting for ${description}`);
}

async function resolveChromeExecutable() {
  for (const candidate of COMMON_CHROME_PATHS) {
    try {
      await fs.access(candidate);
      return candidate;
    } catch {
      // ignore
    }
  }
  throw new Error(
    'No Chrome/Chromium executable found. Set BPANE_BENCHMARK_CHROME to a local Chrome path.',
  );
}

async function resolveCertSpki(options) {
  if (options.certSpki?.trim()) {
    return options.certSpki.trim();
  }
  try {
    const value = await fs.readFile(
      new URL('../../../../dev/certs/cert-fingerprint.txt', import.meta.url),
      'utf8',
    );
    return value.trim();
  } catch {
    return '';
  }
}

async function fetchAuthConfig(options) {
  try {
    const response = await fetch(new URL('/auth-config.json', options.pageUrl));
    if (!response.ok) {
      return null;
    }
    return await response.json();
  } catch {
    return null;
  }
}

async function configurePage(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => Boolean(window.__bpaneAuth));
}

async function ensureLoggedIn(page, options) {
  const authConfig = await fetchAuthConfig(options);
  if (!authConfig || authConfig.mode !== 'oidc') {
    return authConfig;
  }

  const state = await page.evaluate(() => ({
    configured: window.__bpaneAuth?.isConfigured?.() ?? false,
    authenticated: window.__bpaneAuth?.isAuthenticated?.() ?? false,
    exampleUser: window.__bpaneAuth?.getExampleUser?.() ?? null,
  }));
  if (!state.configured || state.authenticated) {
    return authConfig;
  }

  const exampleUser = state.exampleUser;
  if (!exampleUser?.username || !exampleUser?.password) {
    throw new Error('OIDC auth is enabled, but no example user is configured for smoke login.');
  }

  await page.click('#btn-login');
  const username = page.locator('input[name="username"], #username').first();
  const password = page.locator('input[name="password"], #password').first();
  const loginState = await poll(
    'OIDC login readiness',
    async () => ({
      authenticated: await page
        .evaluate(() => window.__bpaneAuth?.isAuthenticated?.() === true)
        .catch(() => false),
      usernameVisible: await username.isVisible().catch(() => false),
    }),
    (value) => value.authenticated || value.usernameVisible,
    options.connectTimeoutMs,
  );
  if (!loginState.authenticated) {
    await username.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await password.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await username.fill(exampleUser.username);
    await password.fill(exampleUser.password);
    await page.locator('input[type="submit"], #kc-login').click();
  }

  const pageUrl = new URL(options.pageUrl);
  const targetPrefix = `${pageUrl.origin}${pageUrl.pathname}`;
  await page.waitForURL(new RegExp(`^${targetPrefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`), {
    timeout: options.connectTimeoutMs,
  });
  await page.waitForFunction(() => window.__bpaneAuth?.isAuthenticated?.() === true, {
    timeout: options.connectTimeoutMs,
  });
  return authConfig;
}

async function getAccessToken(page) {
  return await page.evaluate(() => window.__bpaneAuth?.getAccessToken?.() ?? null);
}

async function fetchJson(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return await response.json();
}

async function fetchBytes(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

async function inspectZipEntries(bytes) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-workflow-source-'));
  const archivePath = path.join(tempDir, 'source.zip');
  try {
    await fs.writeFile(archivePath, bytes);
    const output = execFileSync(
      'python3',
      [
        '-c',
        'import json, sys, zipfile; archive = zipfile.ZipFile(sys.argv[1]); print(json.dumps(archive.namelist()))',
        archivePath,
      ],
      { encoding: 'utf8' },
    );
    return JSON.parse(output);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-smoke-export',
      description: 'Validate workflow definitions and runs',
      labels: {
        suite: 'smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'openapi/bpane-control-v1.yaml',
      source: {
        kind: 'git',
        repository_url: 'https://github.com/ITmedes/browserpane.git',
        ref: 'refs/heads/main',
        root_path: 'openapi',
      },
      input_schema: {
        type: 'object',
        required: ['month'],
      },
      output_schema: {
        type: 'object',
        required: ['csv_file_id'],
      },
      default_session: {
        labels: {
          origin: 'workflow-smoke',
        },
      },
      allowed_credential_binding_ids: ['cred_smoke'],
      allowed_file_workspace_ids: ['ws_smoke'],
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      input: {
        month: '2026-03',
      },
      labels: {
        suite: 'smoke',
      },
    }),
  });
}

async function fetchWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunWithAutomationToken(automationToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: { 'x-bpane-automation-access-token': automationToken },
  });
}

async function issueAutomationAccess(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/automation-access`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunEvents(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/events`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunEventsWithAutomationToken(automationToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/events`, {
    headers: { 'x-bpane-automation-access-token': automationToken },
  });
}

async function fetchWorkflowRunLogs(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchWorkflowRunLogsWithAutomationToken(automationToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    headers: { 'x-bpane-automation-access-token': automationToken },
  });
}

async function transitionWorkflowRun(automationToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/state`, {
    method: 'POST',
    headers: {
      'x-bpane-automation-access-token': automationToken,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function appendWorkflowRunLog(automationToken, options, runId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    method: 'POST',
    headers: {
      'x-bpane-automation-access-token': automationToken,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function appendAutomationTaskLog(automationToken, options, taskId, body) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks/${taskId}/logs`, {
    method: 'POST',
    headers: {
      'x-bpane-automation-access-token': automationToken,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  });
}

async function fetchSession(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function deleteSession(accessToken, options, sessionId) {
  const response = await fetch(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    method: 'DELETE',
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok && response.status !== 404) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const executablePath = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);
  const chromeArgs = [
    '--origin-to-force-quic-on=localhost:4433',
    '--disable-background-timer-throttling',
    '--disable-renderer-backgrounding',
    '--disable-backgrounding-occluded-windows',
  ];
  if (certSpki) {
    chromeArgs.push(`--ignore-certificate-errors-spki-list=${certSpki}`);
  }

  const browser = await chromium.launch({
    headless: options.headless,
    executablePath,
    args: chromeArgs,
  });

  let context = null;
  let page = null;
  let accessToken = '';
  let automationToken = '';
  let createdSessionId = '';

  try {
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });
    page = await context.newPage();
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    accessToken = (await getAccessToken(page)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    log('Creating workflow definition and immutable version');
    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(accessToken, options, workflow.id);
    if (version.version !== 'v1') {
      throw new Error(`Expected workflow version v1, got ${version.version ?? 'missing'}.`);
    }
    if (version.source?.kind !== 'git' || !version.source?.resolved_commit) {
      throw new Error('Workflow version did not expose resolved git source metadata.');
    }

    log('Creating workflow run');
    const createdRun = await createWorkflowRun(accessToken, options, workflow.id);
    const runId = createdRun.id;
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const pendingRun = await poll(
      'workflow run visibility',
      () => fetchWorkflowRun(accessToken, options, runId),
      (run) => run?.state === 'pending',
      options.connectTimeoutMs,
    );
    if (pendingRun.workflow_version !== 'v1') {
      throw new Error(`Expected workflow run version v1, got ${pendingRun.workflow_version}.`);
    }
    if (!pendingRun.source_snapshot?.content_path) {
      throw new Error('Workflow run did not expose a source snapshot download path.');
    }

    const session = await fetchSession(accessToken, options, createdSessionId);
    if (session.labels?.origin !== 'workflow-smoke') {
      throw new Error('Workflow-created session is missing the workflow-smoke origin label.');
    }

    const issuedAutomationAccess = await issueAutomationAccess(
      accessToken,
      options,
      createdSessionId,
    );
    automationToken = issuedAutomationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Failed to acquire a session automation access token.');
    }
    const sourceSnapshotBytes = await fetchBytes(
      `${options.pageUrl}${pendingRun.source_snapshot.content_path}`,
      {
        headers: {
          'x-bpane-automation-access-token': automationToken,
        },
      },
    );
    if (sourceSnapshotBytes.length === 0) {
      throw new Error('Workflow source snapshot download returned an empty archive.');
    }
    const sourceSnapshotEntries = await inspectZipEntries(sourceSnapshotBytes);
    if (!sourceSnapshotEntries.includes('openapi/bpane-control-v1.yaml')) {
      throw new Error('Workflow source snapshot archive is missing the pinned entrypoint file.');
    }
    if (sourceSnapshotEntries.includes('README.md')) {
      throw new Error('Workflow source snapshot archive leaked files outside the configured root_path.');
    }

    log(`Driving workflow run ${runId} through automation access`);
    await transitionWorkflowRun(automationToken, options, runId, {
      state: 'running',
      message: 'workflow executor attached',
    });
    await appendWorkflowRunLog(automationToken, options, runId, {
      stream: 'system',
      message: 'workflow bootstrapped',
    });
    await appendAutomationTaskLog(automationToken, options, pendingRun.automation_task_id, {
      stream: 'stdout',
      message: 'opened report page',
    });
    await transitionWorkflowRun(automationToken, options, runId, {
      state: 'succeeded',
      output: {
        csv_file_id: 'file_123',
      },
      artifact_refs: ['artifact://workflow-trace.zip'],
      message: 'workflow completed',
    });

    const succeededRun = await poll(
      'workflow run success',
      () => fetchWorkflowRunWithAutomationToken(automationToken, options, runId),
      (run) => run?.state === 'succeeded',
      options.connectTimeoutMs,
    );
    if (succeededRun.output?.csv_file_id !== 'file_123') {
      throw new Error('Workflow run did not persist the expected structured output.');
    }

    const events = await fetchWorkflowRunEventsWithAutomationToken(
      automationToken,
      options,
      runId,
    );
    const eventTypes = events.events.map((event) => event.event_type);
    for (const expected of [
      'workflow_run.created',
      'automation_task.created',
      'workflow_run.running',
      'automation_task.running',
      'workflow_run.succeeded',
      'automation_task.succeeded',
    ]) {
      if (!eventTypes.includes(expected)) {
        throw new Error(`Workflow run events are missing ${expected}.`);
      }
    }

    const logs = await fetchWorkflowRunLogsWithAutomationToken(
      automationToken,
      options,
      runId,
    );
    const logSources = logs.logs.map((log) => log.source);
    if (!logSources.includes('run') || !logSources.includes('automation_task')) {
      throw new Error('Workflow run logs did not expose both run and automation task sources.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      runId,
      state: succeededRun.state,
      sessionId: createdSessionId,
      automationTaskId: succeededRun.automation_task_id,
      sourceSnapshotBytes: sourceSnapshotBytes.length,
      sourceSnapshotEntries: sourceSnapshotEntries.length,
      events: events.events.length,
      logs: logs.logs.length,
      outputCsvFileId: succeededRun.output?.csv_file_id ?? null,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }

    console.log(JSON.stringify(summary, null, 2));
  } finally {
    if (createdSessionId && accessToken) {
      try {
        await deleteSession(accessToken, options, createdSessionId);
      } catch (error) {
        log(
          `cleanup warning: failed to delete session ${createdSessionId}: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }
    if (context) {
      await context.close().catch(() => {});
    }
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(`[workflow-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`);
  process.exitCode = 1;
});
