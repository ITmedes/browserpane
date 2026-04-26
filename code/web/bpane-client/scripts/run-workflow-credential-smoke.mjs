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
Usage: node scripts/run-workflow-credential-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[workflow-credential-smoke] ${message}`);
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
  const repoDir = await fs.mkdtemp(path.join(PROJECT_ROOT, '.workflow-credential-smoke-repo-'));
  const workflowDir = path.join(repoDir, 'workflows', 'credentials');
  await fs.mkdir(workflowDir, { recursive: true });
  await fs.writeFile(
    path.join(workflowDir, 'run.mjs'),
    `export default async function run({
  page,
  credentialBindings,
  credentials,
  sessionId,
  workflowRunId,
  automationTaskId,
}) {
  const binding = credentialBindings[0];
  if (!binding) {
    throw new Error('Expected one credential binding');
  }
  const resolved = await credentials.load(binding.id, 'http://web:8080');
  const payload = resolved.payload ?? {};
  if (typeof payload.username !== 'string' || typeof payload.password !== 'string') {
    throw new Error('Credential payload is missing username/password');
  }
  console.log(\`workflow loaded credential binding \${binding.name}\`);
  await page.goto('http://web:8080/workflow-credential-fixture.html', { waitUntil: 'networkidle' });
  await page.fill('#username', payload.username);
  await page.fill('#password', payload.password);
  await page.click('button[type="submit"]');
  await page.waitForFunction(() => document.title === 'Credential Fixture Authenticated');
  return {
    title: await page.title(),
    final_url: page.url(),
    bound_credential_name: binding.name,
    username: payload.username,
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
  runGitCommand(repoDir, ['commit', '-m', 'Add workflow credential smoke entrypoint']);
  const commit = runGitCommand(repoDir, ['rev-parse', 'HEAD']).trim();
  return {
    repoDir,
    repositoryUrl: `/workspace/${path.basename(repoDir)}`,
    commit,
  };
}

async function createCredentialBinding(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/credential-bindings`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: 'demo-login',
      provider: 'vault_kv_v2',
      namespace: 'smoke',
      allowed_origins: ['http://web:8080'],
      injection_mode: 'form_fill',
      secret_payload: {
        username: 'demo',
        password: 'demo-demo',
      },
      labels: {
        suite: 'workflow-credential-smoke',
      },
    }),
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
      name: 'workflow-credential-smoke',
      description: 'Validate workflow credential bindings',
      labels: {
        suite: 'workflow-credential-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, source, credentialBindingId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/credentials/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
        root_path: 'workflows',
      },
      allowed_credential_binding_ids: [credentialBindingId],
      default_session: {
        labels: {
          origin: 'workflow-credential-smoke',
        },
      },
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId, credentialBindingId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      credential_binding_ids: [credentialBindingId],
      labels: {
        suite: 'workflow-credential-smoke',
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

async function fetchResolvedCredentialBinding(automationToken, options, runId, bindingId) {
  return await fetchJson(
    `${options.pageUrl}/api/v1/workflow-runs/${runId}/credential-bindings/${bindingId}/resolved`,
    {
      headers: { 'x-bpane-automation-access-token': automationToken },
    },
  );
}

async function fetchWorkflowRunLogs(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}/logs`, {
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

    log('Creating credential binding');
    const credentialBinding = await createCredentialBinding(accessToken, options);
    const credentialBindingId = credentialBinding.id;
    if (!credentialBindingId) {
      throw new Error('Credential binding creation did not return an id.');
    }

    log('Creating workflow definition and immutable version');
    const workflow = await createWorkflow(accessToken, options);
    const version = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      localWorkflowSource,
      credentialBindingId,
    );
    if (version.source?.resolved_commit !== localWorkflowSource.commit) {
      throw new Error('Workflow version did not pin the expected local git commit.');
    }

    log('Creating workflow run with a credential binding');
    const createdRun = await createWorkflowRun(
      accessToken,
      options,
      workflow.id,
      credentialBindingId,
    );
    const runId = createdRun.id;
    createdSessionId = createdRun.session_id ?? '';
    if (!runId || !createdSessionId) {
      throw new Error('Workflow run creation did not return run and session ids.');
    }

    const initialRun = await poll(
      'workflow run visibility',
      () => fetchWorkflowRun(accessToken, options, runId),
      (run) => Array.isArray(run?.credential_bindings) && run.credential_bindings.length === 1,
      options.connectTimeoutMs,
    );
    const attachedBinding = initialRun.credential_bindings?.[0];
    if (!attachedBinding?.resolve_path) {
      throw new Error('Workflow run did not expose a credential binding resolve path.');
    }

    const automationAccess = await issueAutomationAccess(accessToken, options, createdSessionId);
    const automationToken = automationAccess.token ?? '';
    if (!automationToken) {
      throw new Error('Failed to acquire a session automation access token.');
    }

    const resolvedCredential = await fetchResolvedCredentialBinding(
      automationToken,
      options,
      runId,
      credentialBindingId,
    );
    if (resolvedCredential.payload?.username !== 'demo') {
      throw new Error('Resolved credential payload did not expose the expected username.');
    }

    log(`Waiting for control-plane workflow execution of run ${runId}`);
    const succeededRun = await poll(
      'workflow run success',
      () => fetchWorkflowRunWithAutomationToken(automationToken, options, runId),
      (run) => run?.state === 'succeeded',
      options.connectTimeoutMs,
    );
    if (succeededRun.output?.title !== 'Credential Fixture Authenticated') {
      throw new Error('Workflow run did not persist the credential fixture title.');
    }
    if (succeededRun.output?.bound_credential_name !== 'demo-login') {
      throw new Error('Workflow run did not persist the bound credential name.');
    }
    if (succeededRun.output?.username !== 'demo') {
      throw new Error('Workflow run did not persist the credential payload username.');
    }
    if (
      typeof succeededRun.output?.final_url !== 'string' ||
      !succeededRun.output.final_url.startsWith('http://web:8080/workflow-credential-fixture.html')
    ) {
      throw new Error('Workflow run did not finish on the credential fixture page.');
    }

    const logs = await fetchWorkflowRunLogs(accessToken, options, runId);
    const logMessages = logs.logs.map((entry) => entry.message);
    if (!logMessages.some((message) => message.includes('materialized workflow credential binding'))) {
      throw new Error('Workflow run logs are missing credential materialization evidence.');
    }
    if (!logMessages.some((message) => message.includes('workflow loaded credential binding demo-login'))) {
      throw new Error('Workflow logs are missing workflow credential usage evidence.');
    }

    const summary = {
      workflowId: workflow.id,
      workflowVersion: 'v1',
      workflowSourceCommit: version.source?.resolved_commit ?? null,
      credentialBindingId,
      runId,
      state: succeededRun.state,
      sessionId: createdSessionId,
      automationTaskId: succeededRun.automation_task_id,
      outputTitle: succeededRun.output?.title ?? null,
      outputFinalUrl: succeededRun.output?.final_url ?? null,
      boundCredentialName: succeededRun.output?.bound_credential_name ?? null,
      username: succeededRun.output?.username ?? null,
      logs: logs.logs.length,
    };

    const prettySummary = JSON.stringify(summary, null, 2);
    console.log(prettySummary);
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${prettySummary}\n`, 'utf8');
    }
  } finally {
    if (accessToken && createdSessionId) {
      await deleteSession(accessToken, options, createdSessionId).catch(() => {});
    }
    if (localWorkflowSource?.repoDir) {
      await fs.rm(localWorkflowSource.repoDir, { recursive: true, force: true }).catch(() => {});
    }
    await context?.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(
    `[workflow-credential-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
