import fs from 'node:fs/promises';
import process from 'node:process';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { chromium } from 'playwright-core';
import {
  cleanupWorkflowSmokeSessions,
  configurePage,
  createLogger,
  fetchJson,
  getAccessToken,
  launchChrome,
  parseSmokeArgs,
  poll,
  PROJECT_ROOT,
  ensureLoggedIn,
} from './workflow-smoke-lib.mjs';

const execFileAsync = promisify(execFile);
const DOCKER_NETWORK = process.env.BPANE_DOCKER_NETWORK || 'deploy_bpane-internal';

async function waitForEmbedControl(page, options) {
  await page.waitForFunction(
    () => Boolean(window.__bpaneControl && window.__bpaneBrowserPolicy),
    { timeout: options.connectTimeoutMs },
  );
}

async function startPolicyProbeSession(page, accessToken, options, log) {
  await cleanupWorkflowSmokeSessions(accessToken, options, log);
  await page.evaluate(async () => {
    await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
  });
  await page.evaluate(async () => {
    await window.__bpaneControl.startNewSession();
  });
  return await poll(
    'browser policy smoke session connection',
    async () => await page.evaluate(() => window.__bpaneControl.getState()),
    (state) => state?.connected === true && Boolean(state?.sessionId),
    options.connectTimeoutMs,
  );
}

async function runCdpPolicyProbe(cdpEndpoint) {
  const args = [
    'run',
    '--rm',
    '--network',
    DOCKER_NETWORK,
    '-v',
    `${PROJECT_ROOT}:/workspace:ro`,
    '-w',
    '/workspace/code/web/bpane-client',
    'node:22-slim',
    'node',
    'scripts/cdp-local-file-policy-probe.mjs',
    '--cdp-endpoint',
    cdpEndpoint,
  ];
  let stdout = '';
  let stderr = '';
  try {
    const output = await execFileAsync('docker', args, {
      maxBuffer: 1024 * 1024,
    });
    stdout = output.stdout;
    stderr = output.stderr;
  } catch (error) {
    stdout = typeof error?.stdout === 'string' ? error.stdout : '';
    stderr = typeof error?.stderr === 'string' ? error.stderr : '';
    throw new Error(
      [
        `CDP policy probe command failed: docker ${args.join(' ')}`,
        stdout ? `stdout:\n${stdout}` : '',
        stderr ? `stderr:\n${stderr}` : '',
      ].filter(Boolean).join('\n'),
    );
  }
  if (stderr.trim()) {
    console.error(stderr.trim());
  }
  return JSON.parse(stdout);
}

async function run() {
  const options = parseSmokeArgs(process.argv.slice(2), 'run-browser-policy-smoke.mjs');
  const log = createLogger('browser-policy-smoke');
  const browser = await launchChrome(chromium, options);
  const context = await browser.newContext({
    viewport: { width: 1440, height: 980 },
  });
  const page = await context.newPage();

  try {
    log(`Opening ${options.pageUrl}`);
    await configurePage(page, options);
    await waitForEmbedControl(page, options);
    await ensureLoggedIn(page, options);
    const accessToken = await getAccessToken(page);
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the test page.');
    }

    const state = await startPolicyProbeSession(page, accessToken, options, log);
    const sessionId = state.sessionId;
    const session = await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    const cdpEndpoint = session?.runtime?.cdp_endpoint;
    if (typeof cdpEndpoint !== 'string' || !cdpEndpoint) {
      throw new Error(`Session ${sessionId} did not expose a runtime CDP endpoint.`);
    }

    log(`Running local-file policy probe through ${cdpEndpoint}`);
    const probe = await runCdpPolicyProbe(cdpEndpoint);
    await page.evaluate((result) => {
      window.__bpaneBrowserPolicy.setProbeResult(result);
    }, probe);
    const harnessState = await page.evaluate(() => window.__bpaneBrowserPolicy.getState());

    if (!probe.blocked) {
      throw new Error(`Expected local file probe to be blocked: ${probe.reason || 'content exposed'}`);
    }
    if (harnessState.mode !== 'deny_all') {
      throw new Error(`Expected harness policy mode deny_all, got ${harnessState.mode}`);
    }
    if (harnessState.fileUrlNavigation !== 'blocked') {
      throw new Error(`Expected harness file URL mode blocked, got ${harnessState.fileUrlNavigation}`);
    }

    const summary = {
      pageUrl: options.pageUrl,
      sessionId,
      runtime: session.runtime,
      probe,
      harnessState,
    };
    console.log(JSON.stringify(summary, null, 2));
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
      log(`Wrote summary to ${options.outputPath}`);
    }
  } finally {
    try {
      await page.evaluate(async () => {
        const state = window.__bpaneControl?.getState?.();
        if (state?.connected) {
          await window.__bpaneControl.disconnect();
        }
      });
    } catch {
      // Ignore cleanup failures so the primary assertion remains visible.
    }
    try {
      const accessToken = await getAccessToken(page);
      if (accessToken) {
        await cleanupWorkflowSmokeSessions(accessToken, options, log);
      }
    } catch {
      // Ignore cleanup failures.
    }
    await context.close();
    await browser.close();
  }
}

run().catch((error) => {
  console.error(`[browser-policy-smoke] ${error.stack || error.message}`);
  process.exitCode = 1;
});
