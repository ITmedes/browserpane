import fs from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import process from 'node:process';
import { chromium } from 'playwright-core';
import {
  cleanupAdminBeforeRun,
  cleanupAdminSmoke,
  ensureAdminLoggedIn,
  getAdminAccessToken,
  openAdminTab,
  waitForBrowserConnected,
} from './admin-smoke-lib.mjs';
import {
  DEFAULTS,
  buildWorkflowWorkerImage,
  createLogger,
  fetchJson,
  launchChrome,
  parseSmokeArgs,
  PROJECT_ROOT,
  poll,
} from './workflow-smoke-lib.mjs';

const ENTRYPOINT_PATH = 'dev/workflows/browserpane-tour/run.mjs';

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-admin-browserpane-tour-workflow.mjs');
  if (options.pageUrl === DEFAULTS.pageUrl) {
    options.pageUrl = `${DEFAULTS.pageUrl}/admin/`;
  }
  if (options.connectTimeoutMs === DEFAULTS.connectTimeoutMs) {
    options.connectTimeoutMs = 240000;
  }
  const rootUrl = new URL('/', options.pageUrl).origin;
  const log = createLogger('admin-browserpane-tour');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({ viewport: { width: 1440, height: 980 } });
  const page = await context.newPage();

  try {
    log(`Opening ${options.pageUrl}`);
    await ensureAdminLoggedIn(page, options);
    await cleanupAdminBeforeRun(page, options, log);
    const accessToken = await getAdminAccessToken(page);
    const source = createWorkspaceSource();
    buildWorkflowWorkerImage();
    const workflow = await createWorkflow(accessToken, rootUrl);
    await createWorkflowVersion(accessToken, rootUrl, workflow.id, source);
    await createAndJoinSession(page, options);
    const runId = await invokeWorkflow(page, options, workflow.id);
    const state = await waitForTerminalState(page, options);
    const runResource = await fetchWorkflowRun(accessToken, rootUrl, runId);
    if (state !== 'succeeded') {
      throw new Error(
        `BrowserPane tour workflow finished in ${state}${runResource.error ? `: ${runResource.error}` : ''}`,
      );
    }
    const summary = {
      workflowId: workflow.id,
      runId,
      state,
      finalUrl: runResource.output?.final_url ?? null,
      visited: runResource.output?.visited ?? [],
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    await cleanupAdminSmoke(page, options, log);
    await context.close();
    await browser.close();
  }
}

function createWorkspaceSource() {
  const ref = execFileSync('git', ['rev-parse', '--abbrev-ref', 'HEAD'], {
    cwd: PROJECT_ROOT,
    encoding: 'utf8',
  }).trim();
  if (!ref || ref === 'HEAD') {
    throw new Error('BrowserPane tour workflow requires a named git branch for /workspace source');
  }
  try {
    execFileSync('git', ['cat-file', '-e', `HEAD:${ENTRYPOINT_PATH}`], {
      cwd: PROJECT_ROOT,
      stdio: 'ignore',
    });
  } catch {
    throw new Error(
      `BrowserPane tour workflow source ${ENTRYPOINT_PATH} must be committed before registering it from /workspace`,
    );
  }
  return { repositoryUrl: '/workspace', ref: `refs/heads/${ref}` };
}

async function createWorkflow(accessToken, rootUrl) {
  return await fetchJson(`${rootUrl}/api/v1/workflows`, {
    method: 'POST',
    headers: jsonHeaders(accessToken),
    body: JSON.stringify({
      name: `browserpane-tour-${Date.now()}`,
      description: 'Example workflow that tours browserpane.io and the GitHub repository',
      labels: { suite: 'admin-browserpane-tour' },
    }),
  });
}

async function createWorkflowVersion(accessToken, rootUrl, workflowId, source) {
  return await fetchJson(`${rootUrl}/api/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    headers: jsonHeaders(accessToken),
    body: JSON.stringify({
      version: 'v1',
      executor: 'playwright',
      entrypoint: ENTRYPOINT_PATH,
      source: {
        kind: 'git',
        repository_url: source.repositoryUrl,
        ref: source.ref,
        root_path: 'dev',
      },
      input_schema: {
        type: 'object',
        properties: {
          scroll_delay_ms: { type: 'number' },
          scroll_step_px: { type: 'number' },
          max_scroll_steps: { type: 'number' },
        },
      },
    }),
  });
}

async function createAndJoinSession(page, options) {
  await openAdminTab(page, 'sessions');
  await page.getByTestId('session-new').click();
  await page.getByTestId('session-row').first().waitFor({
    state: 'visible',
    timeout: options.connectTimeoutMs,
  });
  await waitForBrowserConnected(page, options);
}

async function invokeWorkflow(page, options, workflowId) {
  await openAdminTab(page, 'workflows');
  await page.getByTestId('workflow-refresh').click();
  await waitForWorkflowOption(page, options, workflowId);
  await page.getByTestId('workflow-definition-select').selectOption(workflowId);
  await waitForEnabled(page.getByTestId('workflow-invoke'), options, 'workflow invoke');
  await page.getByTestId('workflow-input').fill('{\n  "scroll_delay_ms": 180,\n  "scroll_step_px": 260\n}');
  await page.getByTestId('workflow-invoke').click();
  return await poll('workflow run id', async () => {
    const text = await page.getByTestId('workflow-run-id').textContent();
    return text?.replace(/^.*Run id:\s*/, '').trim() ?? '';
  }, (value) => Boolean(value && value !== '--'), options.connectTimeoutMs);
}

async function waitForWorkflowOption(page, options, workflowId) {
  await poll('workflow option', async () => {
    return await page.getByTestId('workflow-definition-select').evaluate(
      (select, id) => Array.from(select.options).some((option) => option.value === id),
      workflowId,
    );
  }, Boolean, options.connectTimeoutMs);
}

async function waitForTerminalState(page, options) {
  return await poll('workflow terminal state', async () => {
    await page.getByTestId('workflow-run-refresh').click().catch(() => {});
    return await page.getByTestId('workflow-run-state').textContent();
  }, (state) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(state ?? ''), options.connectTimeoutMs, 1000);
}

async function waitForEnabled(locator, options, description) {
  await poll(description, async () => await locator.isEnabled(), Boolean, options.connectTimeoutMs);
}

async function fetchWorkflowRun(accessToken, rootUrl, runId) {
  return await fetchJson(`${rootUrl}/api/v1/workflow-runs/${encodeURIComponent(runId)}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

function jsonHeaders(accessToken) {
  return { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' };
}

run().catch((error) => {
  console.error(`[admin-browserpane-tour] ${error.stack || error.message}`);
  process.exitCode = 1;
});
