import fs from 'node:fs/promises';
import process from 'node:process';
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
Usage: node scripts/run-automation-task-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[automation-task-smoke] ${message}`);
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

async function fetchAutomationTask(accessToken, options, taskId) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks/${taskId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function listAutomationTasks(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function createAutomationTask(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      display_name: 'Automation Task Smoke',
      executor: 'playwright',
      session: {
        create_session: {
          owner_mode: 'collaborative',
          idle_timeout_sec: 300,
          labels: {
            origin: 'automation-task-smoke',
          },
          recording: {
            mode: 'disabled',
            format: 'webm',
          },
          integration_context: {
            source: 'run-automation-task-smoke',
            origin: new URL(options.pageUrl).origin,
          },
        },
      },
      input: {
        smoke: true,
        step: 'bootstrap',
      },
      labels: {
        suite: 'smoke',
      },
    }),
  });
}

async function fetchAutomationTaskEvents(accessToken, options, taskId) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks/${taskId}/events`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchAutomationTaskLogs(accessToken, options, taskId) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks/${taskId}/logs`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchSession(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function cancelAutomationTask(accessToken, options, taskId) {
  return await fetchJson(`${options.pageUrl}/api/v1/automation-tasks/${taskId}/cancel`, {
    method: 'POST',
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

    log('Creating automation task with a fresh session binding');
    const createdTask = await createAutomationTask(accessToken, options);
    const taskId = createdTask.id;
    createdSessionId = createdTask.session?.session_id ?? '';
    if (!taskId || !createdSessionId) {
      throw new Error('Automation task creation did not return task and session ids.');
    }

    const listedTasks = await listAutomationTasks(accessToken, options);
    const visibleTask = listedTasks.tasks.find((task) => task.id === taskId);
    if (!visibleTask) {
      throw new Error(`Automation task ${taskId} was not returned from listAutomationTasks.`);
    }

    const pendingTask = await poll(
      'automation task visibility',
      () => fetchAutomationTask(accessToken, options, taskId),
      (task) => task?.state === 'pending',
      options.connectTimeoutMs,
    );
    if (pendingTask.session?.source !== 'created_session') {
      throw new Error(
        `Expected created_session binding, got ${pendingTask.session?.source ?? 'missing'}`,
      );
    }

    const createdSession = await fetchSession(accessToken, options, createdSessionId);
    if (createdSession.labels?.origin !== 'automation-task-smoke') {
      throw new Error('Created session is missing the automation-task-smoke origin label.');
    }

    const initialEvents = await fetchAutomationTaskEvents(accessToken, options, taskId);
    if (initialEvents.events.length !== 1) {
      throw new Error(
        `Expected exactly one initial task event, got ${initialEvents.events.length}.`,
      );
    }
    if (initialEvents.events[0]?.event_type !== 'automation_task.created') {
      throw new Error(
        `Unexpected initial task event type: ${initialEvents.events[0]?.event_type ?? 'missing'}`,
      );
    }

    log(`Cancelling automation task ${taskId}`);
    await cancelAutomationTask(accessToken, options, taskId);

    const cancelledTask = await poll(
      'automation task cancellation',
      () => fetchAutomationTask(accessToken, options, taskId),
      (task) => task?.state === 'cancelled',
      options.connectTimeoutMs,
    );
    if (!cancelledTask.cancel_requested_at || !cancelledTask.completed_at) {
      throw new Error('Cancelled task is missing cancellation or completion timestamps.');
    }

    const events = await fetchAutomationTaskEvents(accessToken, options, taskId);
    if (events.events.length !== 2) {
      throw new Error(`Expected two task events after cancellation, got ${events.events.length}.`);
    }
    if (events.events[1]?.event_type !== 'automation_task.cancelled') {
      throw new Error(
        `Unexpected terminal task event type: ${events.events[1]?.event_type ?? 'missing'}`,
      );
    }

    const logs = await fetchAutomationTaskLogs(accessToken, options, taskId);
    if (logs.logs.length !== 1) {
      throw new Error(`Expected one task log after cancellation, got ${logs.logs.length}.`);
    }
    if (logs.logs[0]?.stream !== 'system') {
      throw new Error(`Expected system log stream, got ${logs.logs[0]?.stream ?? 'missing'}.`);
    }

    const summary = {
      taskId,
      sessionId: createdSessionId,
      state: cancelledTask.state,
      events: events.events.length,
      logs: logs.logs.length,
      executor: cancelledTask.executor,
      sessionSource: cancelledTask.session.source,
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
  console.error(
    `[automation-task-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`,
  );
  process.exitCode = 1;
});
