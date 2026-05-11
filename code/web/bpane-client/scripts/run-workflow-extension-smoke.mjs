import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { testEmbedPageUrl } from './workflow-smoke-lib.mjs';

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
Usage: node scripts/run-workflow-extension-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[workflow-extension-smoke] ${message}`);
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
  await page.goto(testEmbedPageUrl(options), { waitUntil: 'networkidle' });
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
  const repoDir = await fs.mkdtemp(path.join(PROJECT_ROOT, '.workflow-extension-smoke-repo-'));
  const workflowDir = path.join(repoDir, 'workflows', 'extensions');
  await fs.mkdir(workflowDir, { recursive: true });
  await fs.writeFile(
    path.join(workflowDir, 'run.mjs'),
    `export default async function run({ page, sessionId, workflowRunId, automationTaskId }) {
  await page.goto('http://web:8080/workflow-extension-fixture.html', { waitUntil: 'networkidle' });
  await page.waitForFunction(() => document.body?.dataset?.extensionReady === '1');
  const marker = await page.locator('#bpane-extension-marker').textContent();
  return {
    title: await page.title(),
    final_url: page.url(),
    extension_ready: await page.evaluate(() => document.body?.dataset?.extensionReady ?? null),
    marker_text: marker,
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
  runGitCommand(repoDir, ['commit', '-m', 'Add workflow extension smoke entrypoint']);
  const commit = runGitCommand(repoDir, ['rev-parse', 'HEAD']).trim();
  return {
    repoDir,
    repositoryUrl: `/workspace/${path.basename(repoDir)}`,
    commit,
  };
}

async function createExtension(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/extensions`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-smoke-extension',
      description: 'Extension smoke fixture',
      labels: {
        suite: 'workflow-extension-smoke',
      },
    }),
  });
}

async function createExtensionVersion(accessToken, options, extensionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/extensions/${extensionId}/versions`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      version: '1.0.0',
      install_path: '/home/bpane/bpane-test-extension',
    }),
  });
}

async function createWorkflowDefinition(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      name: 'workflow-extension-smoke',
      description: 'Validates that workflow sessions can load approved extensions',
      labels: {
        suite: 'workflow-extension-smoke',
      },
    }),
  });
}

async function createWorkflowVersion(accessToken, options, workflowId, source, extensionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: 'workflows/extensions/run.mjs',
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: 'refs/heads/main',
      },
      default_session: {
        labels: {
          suite: 'workflow-extension-smoke',
        },
        extension_ids: [extensionId],
      },
      allowed_extension_ids: [extensionId],
    }),
  });
}

async function createWorkflowRun(accessToken, options, workflowId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${accessToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({
      workflow_id: workflowId,
      version: 'v1',
      labels: {
        suite: 'workflow-extension-smoke',
      },
    }),
  });
}

async function getWorkflowRun(accessToken, options, runId) {
  return await fetchJson(`${options.pageUrl}/api/v1/workflow-runs/${runId}`, {
    headers: {
      authorization: `Bearer ${accessToken}`,
    },
  });
}

async function getSession(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: {
      authorization: `Bearer ${accessToken}`,
    },
  });
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const chromeExecutable = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);
  const repo = await createLocalWorkflowRepo();
  let browser;
  try {
    const launchArgs = [];
    if (certSpki) {
      launchArgs.push(`--ignore-certificate-errors-spki-list=${certSpki}`);
    }
    browser = await chromium.launch({
      executablePath: chromeExecutable,
      headless: options.headless,
      args: launchArgs,
    });
    const page = await browser.newPage();
    log(`using browser executable ${chromeExecutable}`);
    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    const accessToken = await getAccessToken(page);
    if (!accessToken) {
      throw new Error('Failed to acquire access token from local auth flow.');
    }

    const extension = await createExtension(accessToken, options);
    const extensionVersion = await createExtensionVersion(accessToken, options, extension.id);
    const workflow = await createWorkflowDefinition(accessToken, options);
    const workflowVersion = await createWorkflowVersion(
      accessToken,
      options,
      workflow.id,
      repo,
      extension.id,
    );
    const run = await createWorkflowRun(accessToken, options, workflow.id);

    const completedRun = await poll(
      'workflow run completion',
      () => getWorkflowRun(accessToken, options, run.id),
      (candidate) =>
        ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(candidate.state),
      options.connectTimeoutMs,
      1000,
    );

    if (completedRun.state !== 'succeeded') {
      throw new Error(
        `Workflow run ${completedRun.id} ended in state ${completedRun.state}: ${completedRun.error ?? 'unknown error'}`,
      );
    }

    const session = await getSession(accessToken, options, completedRun.session_id);
    const extensionOutput = completedRun.output ?? {};
    if (extensionOutput.extension_ready !== '1') {
      throw new Error('Workflow output did not report extension_ready=1');
    }
    if (extensionOutput.title !== 'Workflow Extension Fixture Activated') {
      throw new Error(`Unexpected workflow output title: ${extensionOutput.title}`);
    }
    if (!completedRun.extensions?.length) {
      throw new Error('Workflow run did not expose any applied extensions');
    }
    if (!session.extensions?.length) {
      throw new Error('Session did not expose any applied extensions');
    }

    const summary = {
      extensionId: extension.id,
      extensionVersionId: extensionVersion.id,
      workflowId: workflow.id,
      workflowVersion: workflowVersion.version,
      workflowSourceCommit: workflowVersion.source?.resolved_commit ?? null,
      workflowRunId: completedRun.id,
      sessionId: completedRun.session_id,
      state: completedRun.state,
      extensionName: completedRun.extensions[0]?.name ?? null,
      extensionVersion: completedRun.extensions[0]?.version ?? null,
      outputTitle: extensionOutput.title ?? null,
      markerText: extensionOutput.marker_text ?? null,
      finalUrl: extensionOutput.final_url ?? null,
    };

    log(`workflow run ${summary.workflowRunId} completed with extension ${summary.extensionName}`);
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
  } finally {
    await browser?.close().catch(() => {});
    await fs.rm(repo.repoDir, { recursive: true, force: true }).catch(() => {});
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
