import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
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

const PROJECT_ROOT = fileURLToPath(new URL('../../../../', import.meta.url));

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

function runGitCommand(repoDir, args, options = {}) {
  return execFileSync('git', args, {
    cwd: repoDir,
    stdio: ['ignore', 'pipe', 'pipe'],
    encoding: 'utf8',
    ...options,
  });
}

function initializeMainBranch(repoDir) {
  try {
    runGitCommand(repoDir, ['init', '-b', 'main']);
  } catch {
    runGitCommand(repoDir, ['init']);
    runGitCommand(repoDir, ['checkout', '-b', 'main']);
  }
}

async function createLocalWorkflowRepo() {
  const repoDir = await fs.mkdtemp(path.join(PROJECT_ROOT, '.workflow-smoke-repo-'));
  const workflowDir = path.join(repoDir, 'workflows', 'smoke');
  await fs.mkdir(workflowDir, { recursive: true });
  await fs.writeFile(
    path.join(workflowDir, 'run.mjs'),
    `export default async function run({ page, input, sessionId, workflowRunId, automationTaskId }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080';
  console.log(\`workflow visiting \${targetUrl}\`);
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  const title = await page.title();
  console.error(\`workflow captured title \${title}\`);
  return {
    title,
    final_url: page.url(),
    session_id: sessionId,
    workflow_run_id: workflowRunId,
    automation_task_id: automationTaskId,
  };
}
`,
    'utf8',
  );
  initializeMainBranch(repoDir);
  runGitCommand(repoDir, ['config', 'user.name', 'BrowserPane Smoke']);
  runGitCommand(repoDir, ['config', 'user.email', 'smoke@browserpane.local']);
  runGitCommand(repoDir, ['add', '.']);
  runGitCommand(repoDir, ['commit', '-m', 'Add workflow smoke entrypoint']);
  const commit = runGitCommand(repoDir, ['rev-parse', 'HEAD']).trim();
  return {
    repoDir,
    repositoryUrl: `/workspace/${path.basename(repoDir)}`,
    commit,
  };
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

async function createWorkflowVersion(accessToken, options, workflowId, source) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/smoke/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      input_schema: {
        type: 'object',
        required: ['target_url'],
        properties: {
          target_url: {
            type: 'string',
          },
        },
      },
      output_schema: {
        type: 'object',
        required: ['title', 'final_url', 'session_id', 'workflow_run_id', 'automation_task_id'],
      },
      default_session: {
        labels: {
          origin: 'workflow-smoke',
        },
      },
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
        target_url: 'http://web:8080',
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

async function fetchWorkflowRunEventsWithAutomationToken(automationToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/events`, {
    headers: { 'x-bpane-automation-access-token': automationToken },
  });
}

async function fetchWorkflowRunLogsWithAutomationToken(automationToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
    headers: { 'x-bpane-automation-access-token': automationToken },
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

function buildWorkflowWorkerImage() {
  execFileSync(
    'docker',
    ['compose', '-f', 'deploy/compose.yml', '--profile', 'workflow', 'build', 'workflow-worker'],
    {
      cwd: PROJECT_ROOT,
      stdio: 'inherit',
    },
  );
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
  let localWorkflowSource = null;

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

    log('Preparing local git-backed workflow source');
    localWorkflowSource = await createLocalWorkflowRepo();

    log('Building workflow-worker image');
    buildWorkflowWorkerImage();

    log('Creating workflow definition and immutable version');
    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      localWorkflowSource,
    );
    if (version.version !== 'v1') {
      throw new Error(`Expected workflow version v1, got ${version.version ?? 'missing'}.`);
    }
    if (version.source?.kind !== 'git' || !version.source?.resolved_commit) {
      throw new Error('Workflow version did not expose resolved git source metadata.');
    }
    if (version.source.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Workflow version did not pin the expected local git commit.');
    }

    log('Creating workflow run');
    const createdRun = await createWorkflowRun(accessToken, options, workflow.id);
    const runId = createdRun.id;
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const initialRun = await poll(
      'workflow run visibility',
      () => fetchWorkflowRun(accessToken, options, runId),
      (run) => Boolean(run?.source_snapshot?.content_path),
      options.connectTimeoutMs,
    );
    if (initialRun.workflow_version !== 'v1') {
      throw new Error(`Expected workflow run version v1, got ${initialRun.workflow_version}.`);
    }
    if (!initialRun.source_snapshot?.content_path) {
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
      `${options.pageUrl}${initialRun.source_snapshot.content_path}`,
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
    if (!sourceSnapshotEntries.includes('workflows/smoke/run.mjs')) {
      throw new Error('Workflow source snapshot archive is missing the pinned entrypoint file.');
    }
    if (sourceSnapshotEntries.some((entry) => entry.startsWith('.git/'))) {
      throw new Error(
        'Workflow source snapshot archive leaked repository internals outside the configured root_path.',
      );
    }

    log(`Waiting for control-plane workflow execution of run ${runId}`);

    const succeededRun = await poll(
      'workflow run success',
      () => fetchWorkflowRunWithAutomationToken(automationToken, options, runId),
      (run) => run?.state === 'succeeded',
      options.connectTimeoutMs,
    );
    if (succeededRun.output?.title !== 'BrowserPane Test Embed') {
      throw new Error('Workflow run did not persist the expected page title output.');
    }
    if (!String(succeededRun.output?.final_url ?? '').startsWith('http://web:8080')) {
      throw new Error('Workflow run did not persist the expected final URL.');
    }
    if (succeededRun.output?.session_id !== createdSessionId) {
      throw new Error('Workflow run did not persist the expected session id.');
    }
    if (succeededRun.output?.workflow_run_id !== runId) {
      throw new Error('Workflow run did not persist the expected run id.');
    }
    if (succeededRun.output?.automation_task_id !== succeededRun.automation_task_id) {
      throw new Error('Workflow run did not persist the expected automation task id.');
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
      'workflow_run.starting',
      'automation_task.starting',
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
    if (
      !logs.logs.some(
        (log) =>
          log.source === 'run' &&
          log.message.includes('materialized workflow source snapshot'),
      )
    ) {
      throw new Error('Workflow run logs are missing the source snapshot materialization message.');
    }
    if (
      !logs.logs.some(
        (log) =>
          log.source === 'automation_task' &&
          log.message.includes('workflow visiting http://web:8080'),
      )
    ) {
      throw new Error('Workflow run logs are missing the workflow stdout message.');
    }
    if (
      !logs.logs.some(
        (log) =>
          log.source === 'automation_task' &&
          log.message.includes('workflow captured title BrowserPane Test Embed'),
      )
    ) {
      throw new Error('Workflow run logs are missing the workflow stderr message.');
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
      outputTitle: succeededRun.output?.title ?? null,
      outputFinalUrl: succeededRun.output?.final_url ?? null,
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
    if (localWorkflowSource?.repoDir) {
      await fs.rm(localWorkflowSource.repoDir, { recursive: true, force: true }).catch(() => {});
    }
    if (context) {
      await context.close().catch(() => {});
    }
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
