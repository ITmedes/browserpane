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
Usage: node scripts/run-workflow-workspace-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[workflow-workspace-smoke] ${message}`);
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
  const repoDir = await fs.mkdtemp(path.join(PROJECT_ROOT, '.workflow-workspace-smoke-repo-'));
  const workflowDir = path.join(repoDir, 'workflows', 'workspace');
  await fs.mkdir(workflowDir, { recursive: true });
  await fs.writeFile(
    path.join(workflowDir, 'run.mjs'),
    `import { readFile } from 'node:fs/promises';

export default async function run({
  page,
  workspaceInputs,
  sessionId,
  workflowRunId,
  automationTaskId,
}) {
  const inputFile = workspaceInputs?.[0];
  if (!inputFile) {
    throw new Error('workspace input is required');
  }
  const csvText = await readFile(inputFile.localPath, 'utf8');
  const rows = csvText.trim().split(/\\r?\\n/u);
  const values = rows[1]?.split(',') ?? [];
  const total = Number(values[1] ?? NaN);
  console.log(\`workflow loaded workspace input \${inputFile.mountPath}\`);
  await page.goto('http://web:8080', { waitUntil: 'networkidle' });
  const title = await page.title();
  console.error(\`workflow validated workspace input total \${total}\`);
  return {
    title,
    final_url: page.url(),
    workspace_input_mount_path: inputFile.mountPath,
    workspace_input_file_name: inputFile.fileName,
    csv_total: total,
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
  runGitCommand(repoDir, ['commit', '-m', 'Add workflow workspace smoke entrypoint']);
  const commit = runGitCommand(repoDir, ['rev-parse', 'HEAD']).trim();
  return {
    repoDir,
    repositoryUrl: `/workspace/${path.basename(repoDir)}`,
    commit,
  };
}

async function createWorkspace(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/file-workspaces`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-workspace-smoke',
      description: 'Reusable workspace inputs for workflow smoke',
      labels: {
        suite: 'workflow-workspace-smoke',
      },
    }),
  });
}

async function uploadWorkspaceFile(accessToken, options, workspaceId, bytes) {
  return await fetchJson(`${options.pageUrl}/api/v1/file-workspaces/${workspaceId}/files`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'text/csv',
      'x-bpane-file-name': 'monthly-report.csv',
    },
    body: bytes,
  });
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-workspace-smoke',
      description: 'Validate workflow workspace inputs',
      labels: {
        suite: 'workflow-workspace-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, source, workspaceId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/workspace/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      output_schema: {
        type: 'object',
        required: [
          'title',
          'final_url',
          'workspace_input_mount_path',
          'workspace_input_file_name',
          'csv_total',
          'session_id',
          'workflow_run_id',
          'automation_task_id',
        ],
      },
      default_session: {
        labels: {
          origin: 'workflow-workspace-smoke',
        },
      },
      allowed_file_workspace_ids: [workspaceId],
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId, workspaceId, fileId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      workspace_inputs: [
        {
          workspace_id: workspaceId,
          file_id: fileId,
          mount_path: 'inputs/monthly-report.csv',
        },
      ],
      labels: {
        suite: 'workflow-workspace-smoke',
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

    log('Preparing reusable workspace input file');
    const workspace = await createWorkspace(accessToken, options);
    const uploadedFile = await uploadWorkspaceFile(
      accessToken,
      options,
      workspace.id,
      Buffer.from('month,total\n2026-03,42\n', 'utf8'),
    );

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
      workspace.id,
    );
    if (version.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Workflow version did not pin the expected local git commit.');
    }

    log('Creating workflow run with a workspace input');
    const createdRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      workspace.id,
      uploadedFile.id,
    );
    const runId = createdRun.id;
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const initialRun = await poll(
      'workflow run visibility',
      () => fetchWorkflowRun(accessToken, options, runId),
      (run) => Boolean(run?.workspace_inputs?.[0]?.content_path),
      options.connectTimeoutMs,
    );
    const workspaceInput = initialRun.workspace_inputs?.[0];
    if (!workspaceInput) {
      throw new Error('Workflow run did not expose a workspace input resource.');
    }
    if (workspaceInput.mount_path !== 'inputs/monthly-report.csv') {
      throw new Error(`Unexpected workspace input mount path: ${workspaceInput.mount_path}`);
    }

    const issuedAutomationAccess = await issueAutomationAccess(
      accessToken,
      options,
      createdSessionId,
    );
    const automationToken = issuedAutomationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Failed to acquire a session automation access token.');
    }

    const downloadedInput = await fetchBytes(`${options.pageUrl}${workspaceInput.content_path}`, {
      headers: {
        'x-bpane-automation-access-token': automationToken,
      },
    });
    const downloadedInputText = downloadedInput.toString('utf8');
    if (!downloadedInputText.includes('2026-03,42')) {
      throw new Error('Run-scoped workspace input download did not return the expected CSV.');
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
    if (succeededRun.output?.csv_total !== 42) {
      throw new Error('Workflow run did not consume the expected workspace CSV payload.');
    }
    if (succeededRun.output?.workspace_input_mount_path !== 'inputs/monthly-report.csv') {
      throw new Error('Workflow run did not report the expected workspace input mount path.');
    }
    if (succeededRun.output?.workspace_input_file_name !== 'monthly-report.csv') {
      throw new Error('Workflow run did not report the expected workspace input file name.');
    }

    const events = await fetchWorkflowRunEventsWithAutomationToken(
      automationToken,
      options,
      runId,
    );
    const logs = await fetchWorkflowRunLogsWithAutomationToken(
      automationToken,
      options,
      runId,
    );
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'run' &&
          entry.message.includes('materialized workflow workspace input inputs/monthly-report.csv'),
      )
    ) {
      throw new Error('Workflow run logs are missing the workspace input materialization message.');
    }
    if (
      !logs.logs.some(
        (entry) =>
          entry.source === 'automation_task' &&
          entry.message.includes('workflow loaded workspace input inputs/monthly-report.csv'),
      )
    ) {
      throw new Error('Workflow automation task logs are missing the workspace input stdout message.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      workspaceId: workspace.id,
      workspaceFileId: uploadedFile.id,
      runId,
      state: succeededRun.state,
      sessionId: createdSessionId,
      automationTaskId: succeededRun.automation_task_id,
      workspaceInputBytes: downloadedInput.length,
      workspaceInputMountPath: succeededRun.output?.workspace_input_mount_path ?? null,
      workspaceInputFileName: succeededRun.output?.workspace_input_file_name ?? null,
      csvTotal: succeededRun.output?.csv_total ?? null,
      events: events.events.length,
      logs: logs.logs.length,
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
    `[workflow-workspace-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
