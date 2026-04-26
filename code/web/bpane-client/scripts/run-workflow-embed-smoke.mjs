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
Usage: node scripts/run-workflow-embed-smoke.mjs [options]

Options:
  --page-url <url>            Local test-embed URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Timeout budget (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[workflow-embed-smoke] ${message}`);
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
  await page.waitForFunction(
    () => Boolean(window.__bpaneAuth && window.__bpaneWorkflow),
    { timeout: options.connectTimeoutMs },
  );
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
  const repoDir = await fs.mkdtemp(path.join(PROJECT_ROOT, '.workflow-embed-smoke-repo-'));
  const workflowDir = path.join(repoDir, 'workflows', 'embed');
  await fs.mkdir(workflowDir, { recursive: true });
  await fs.writeFile(
    path.join(workflowDir, 'run.mjs'),
    `export default async function run({ page, input, sessionId, workflowRunId, automationTaskId, artifacts }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080/';
  if (!input?.output_workspace_id) {
    throw new Error('workflow embed smoke requires input.output_workspace_id');
  }
  console.log(\`workflow visiting \${targetUrl}\`);
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  const title = await page.title();
  await artifacts.uploadTextFile({
    workspaceId: input.output_workspace_id,
    fileName: 'workflow-embed-summary.txt',
    mediaType: 'text/plain',
    provenance: {
      origin: 'workflow-embed-smoke',
      workflow_run_id: workflowRunId,
      automation_task_id: automationTaskId,
      session_id: sessionId,
    },
    text: \`title=\${title}\\nurl=\${page.url()}\\nsession=\${sessionId}\\nrun=\${workflowRunId}\\n\`,
  });
  console.error(\`workflow captured title \${title}\`);
  return {
    title,
    final_url: page.url(),
    session_id: sessionId,
    workflow_run_id: workflowRunId,
    automation_task_id: automationTaskId,
    output_file_name: 'workflow-embed-summary.txt',
  };
}
`,
    'utf8',
  );
  initializeMainBranch(repoDir);
  runGitCommand(repoDir, ['config', 'user.name', 'BrowserPane Smoke']);
  runGitCommand(repoDir, ['config', 'user.email', 'smoke@browserpane.local']);
  runGitCommand(repoDir, ['add', '.']);
  runGitCommand(repoDir, ['commit', '-m', 'Add workflow embed smoke entrypoint']);
  const commit = runGitCommand(repoDir, ['rev-parse', 'HEAD']).trim();
  return {
    repoDir,
    repositoryUrl: `/workspace/${path.basename(repoDir)}`,
    commit,
  };
}

async function createFileWorkspace(accessToken, options) {
  return await fetchJson(`${options.pageUrl.replace(/\/dev\/test-embed\.html$/, '')}/api/v1/file-workspaces`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-embed-smoke-outputs',
      description: 'Artifacts produced by the workflow embed smoke',
      labels: {
        suite: 'workflow-embed-smoke',
      },
    }),
  });
}

async function createWorkflow(accessToken, options) {
  return await fetchJson(`${options.pageUrl.replace(/\/dev\/test-embed\.html$/, '')}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-embed-smoke',
      description: 'Validate test-embed workflow controls',
      labels: {
        suite: 'workflow-embed-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, source, workspaceId) {
  return await fetchJson(
    `${options.pageUrl.replace(/\/dev\/test-embed\.html$/, '')}/api/v1/workflows/${workflowId}/versions`,
    {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${accessToken}`,
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      body: JSON.stringify({
        version: 'v1',
        executor: 'playwright',
        entrypoint: 'workflows/embed/run.mjs',
        source,
        input_schema: {
          type: 'object',
          required: ['output_workspace_id'],
          properties: {
            target_url: { type: 'string' },
            output_workspace_id: { type: 'string' },
          },
        },
        output_schema: {
          type: 'object',
          required: ['title', 'final_url', 'session_id', 'workflow_run_id', 'output_file_name'],
          properties: {
            title: { type: 'string' },
            final_url: { type: 'string' },
            session_id: { type: 'string' },
            workflow_run_id: { type: 'string' },
            output_file_name: { type: 'string' },
          },
        },
        default_session: {
          labels: {
            origin: 'workflow-embed-smoke',
          },
          recording: {
            mode: 'manual',
            format: 'webm',
          },
        },
        allowed_file_workspace_ids: [workspaceId],
      }),
    },
  );
}

async function buildWorkflowWorkerImage() {
  execFileSync(
    'docker',
    ['compose', '-f', 'deploy/compose.yml', '--profile', 'workflow', 'build', 'workflow-worker'],
    {
      cwd: PROJECT_ROOT,
      stdio: 'inherit',
      env: process.env,
    },
  );
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const chromeExecutable = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);
  const pageUrl = new URL(options.pageUrl);
  const apiBaseUrl = `${pageUrl.origin}`;
  const tempDownloadDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-workflow-embed-downloads-'));

  let browser;
  let context;
  let repoDir = '';

  try {
    log('Preparing local git-backed workflow source');
    const localRepo = await createLocalWorkflowRepo();
    repoDir = localRepo.repoDir;

    log('Building workflow-worker image');
    await buildWorkflowWorkerImage();

    browser = await chromium.launch({
      executablePath: chromeExecutable,
      headless: options.headless,
      args: certSpki
        ? [`--ignore-certificate-errors-spki-list=${certSpki}`]
        : [],
    });
    context = await browser.newContext({
      acceptDownloads: true,
      ignoreHTTPSErrors: true,
    });
    const page = await context.newPage();

    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    const accessToken = await getAccessToken(page);
    if (!accessToken) {
      throw new Error('No access token available after login.');
    }

    log('Creating workflow definition and immutable version');
    const workspace = await createFileWorkspace(accessToken, options);
    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      {
        kind: 'git',
        repository_url: localRepo.repositoryUrl,
        ref: 'HEAD',
        root_path: 'workflows',
      },
      workspace.id,
    );

    log('Driving workflow invocation through test-embed hooks');
    const createdRun = await page.evaluate(
      async ({ workflowId, versionName, outputWorkspaceId }) => {
        await window.__bpaneWorkflow.refreshDefinitions({ preserveSelection: false, silent: true });
        await window.__bpaneWorkflow.selectWorkflow(workflowId, { loadVersion: false });
        window.__bpaneWorkflow.setVersion(versionName);
        await window.__bpaneWorkflow.loadVersion({ silent: true });
        window.__bpaneWorkflow.setInput({
          target_url: 'http://web:8080/',
          output_workspace_id: outputWorkspaceId,
        });
        return await window.__bpaneWorkflow.invokeSelected({ silent: true });
      },
      {
        workflowId: workflow.id,
        versionName: version.version,
        outputWorkspaceId: workspace.id,
      },
    );

    const runState = await poll(
      'workflow run success through test-embed',
      async () =>
        await page.evaluate(async () => {
          if (window.__bpaneWorkflow?.getState?.()?.run?.id) {
            await window.__bpaneWorkflow.refreshRun({ silent: true }).catch(() => {});
          }
          return window.__bpaneWorkflow?.getState?.() ?? null;
        }),
      (value) => value?.run?.state === 'succeeded',
      options.connectTimeoutMs,
      1000,
    );

    const run = runState.run;
    if (!run?.output?.title || run.output.title !== 'BrowserPane Test Embed') {
      throw new Error(`Unexpected workflow output title: ${run?.output?.title ?? 'missing'}`);
    }
    if (!Array.isArray(run.produced_files) || run.produced_files.length !== 1) {
      throw new Error('Expected exactly one produced file in the workflow run state.');
    }

    await page.locator('#workflow-run-status').waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    const workflowStatusText = await page.locator('#workflow-run-status').textContent();
    if (!workflowStatusText?.includes('succeeded')) {
      throw new Error(`Workflow panel did not render success status: ${workflowStatusText ?? 'missing'}`);
    }

    const workflowOutputText = await page.locator('#workflow-output-panel').textContent();
    if (!workflowOutputText?.includes('BrowserPane Test Embed')) {
      throw new Error('Workflow output panel did not render the expected title.');
    }

    await page.waitForFunction(
      () => {
        const panel = document.getElementById('workflow-log-panel');
        return Boolean(panel?.textContent?.includes('workflow visiting http://web:8080'));
      },
      { timeout: options.connectTimeoutMs },
    );
    await page.waitForFunction(
      () => {
        const panel = document.getElementById('workflow-file-list');
        return Boolean(panel?.querySelector('button[data-action="download-workflow-file"]'));
      },
      { timeout: options.connectTimeoutMs },
    );

    log('Downloading workflow artifact through the embed panel');
    const downloadPromise = page.waitForEvent('download', { timeout: options.connectTimeoutMs });
    await page.locator('#workflow-file-list button[data-action="download-workflow-file"]').click();
    const download = await downloadPromise;
    const savedPath = path.join(tempDownloadDir, download.suggestedFilename());
    await download.saveAs(savedPath);
    const downloadedArtifact = await fs.readFile(savedPath, 'utf8');
    if (!downloadedArtifact.includes('title=BrowserPane Test Embed')) {
      throw new Error('Downloaded workflow artifact did not contain the expected title.');
    }

    const versionSummaryText = await page.locator('#workflow-version-summary').textContent();
    if (!versionSummaryText?.includes('entrypoint=workflows/embed/run.mjs')) {
      throw new Error('Workflow version summary did not render the expected entrypoint.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: version.version,
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      workflowRunId: run.id,
      workflowPanelRunId: createdRun?.id ?? null,
      workflowSessionId: run.session_id,
      producedFileId: run.produced_files[0]?.file_id ?? null,
      downloadedArtifactName: download.suggestedFilename(),
      downloadedArtifactBytes: Buffer.byteLength(downloadedArtifact, 'utf8'),
      logs: runState.logs?.length ?? 0,
      events: runState.events?.length ?? 0,
    };

    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
    console.log(JSON.stringify(summary, null, 2));
  } finally {
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
    if (repoDir) {
      await fs.rm(repoDir, { recursive: true, force: true }).catch(() => {});
    }
    await fs.rm(tempDownloadDir, { recursive: true, force: true }).catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-embed-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
