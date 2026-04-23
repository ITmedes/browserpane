import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { chromium } from 'playwright-core';

const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  certSpki: process.env.BPANE_BENCHMARK_CERT_SPKI ?? '',
  connectTimeoutMs: 30000,
  recordDurationMs: 4000,
  headless: false,
  outputPath: '',
  summaryPath: '',
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
    } else if (arg === '--record-duration-ms' && next) {
      options.recordDurationMs = Number(next);
      i++;
    } else if (arg === '--output' && next) {
      options.outputPath = next;
      i++;
    } else if (arg === '--summary' && next) {
      options.summaryPath = next;
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
Usage: node scripts/run-recording-smoke.mjs [options]

Options:
  --page-url <url>            Local test page URL (default: ${DEFAULTS.pageUrl})
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --record-duration-ms <ms>   Capture duration after start (default: ${DEFAULTS.recordDurationMs})
  --output <path>             Save the recorded WebM to this file
  --summary <path>            Write JSON summary to file
  --headless                  Run headless
`);
}

function log(message) {
  console.log(`[recording-smoke] ${message}`);
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

async function configurePage(page, options, pageUrl = options.pageUrl) {
  await page.goto(pageUrl, { waitUntil: 'networkidle' });
  await page.waitForFunction(
    () => Boolean(window.__bpaneAuth && window.__bpaneControl && window.__bpaneRecording),
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

function buildRecorderPageUrl(pageUrl) {
  const url = new URL(pageUrl);
  url.searchParams.set('client_role', 'recorder');
  return url.toString();
}

async function startNewInteractiveSession(page, options) {
  const resource = await page.evaluate(() => window.__bpaneControl.startNewSession({
    clientRole: 'interactive',
  }));
  if (!resource?.id) {
    throw new Error('Failed to create a new session resource.');
  }
  await page.waitForFunction(
    () => window.__bpaneControl?.getState?.()?.connected === true,
    { timeout: options.connectTimeoutMs },
  );
  await page.waitForSelector('#desktop-container canvas', { timeout: options.connectTimeoutMs });
  return resource;
}

async function connectRecorderToSession(page, options, sessionId) {
  await page.evaluate(async (id) => {
    await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
    await window.__bpaneControl.selectSession(id);
    await window.__bpaneControl.connectSelected({ clientRole: 'recorder' });
  }, sessionId);
  await page.waitForFunction(
    () => window.__bpaneControl?.getState?.()?.connected === true,
    { timeout: options.connectTimeoutMs },
  );
  await page.waitForSelector('#desktop-container canvas', { timeout: options.connectTimeoutMs });
  return await page.evaluate(() => window.__bpaneControl.getState());
}

async function disconnectPage(page) {
  await page.evaluate(async () => {
    const state = window.__bpaneControl?.getState?.();
    if (state?.connected) {
      await window.__bpaneControl.disconnect();
    }
  }).catch(() => {});
}

async function nudgeRemotePage(page) {
  const canvas = page.locator('#desktop-container canvas').first();
  await canvas.waitFor({ state: 'visible', timeout: 10000 });
  await canvas.click({ force: true });
  await page.mouse.wheel(0, 1200);
  await sleep(700);
  await page.keyboard.press('PageDown');
  await sleep(700);
  await page.keyboard.press('PageUp');
}

async function fetchJson(url, init) {
  const response = await fetch(url, init);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return await response.json();
}

async function fetchSessionStatus(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/status`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchSessionResource(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function deleteSession(accessToken, options, sessionId) {
  try {
    const response = await fetch(`${options.pageUrl}/api/v1/sessions/${sessionId}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${accessToken}` },
    });
    if (!response.ok && response.status !== 404) {
      const detail = await response.text().catch(() => '');
      throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
    }
  } catch (error) {
    log(
      `cleanup warning: failed to stop session ${sessionId}: ${error instanceof Error ? error.message : String(error)}`,
    );
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
  let ownerPage = null;
  let recorderPage = null;
  let accessToken = '';
  let sessionId = '';
  let outputPath = options.outputPath;

  try {
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
      acceptDownloads: true,
    });

    ownerPage = await context.newPage();
    await configurePage(ownerPage, options, options.pageUrl);
    await ensureLoggedIn(ownerPage, options);
    accessToken = (await getAccessToken(ownerPage)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the owner page.');
    }

    recorderPage = await context.newPage();
    await configurePage(recorderPage, options, buildRecorderPageUrl(options.pageUrl));
    await ensureLoggedIn(recorderPage, options);

    log('Starting source session');
    const createdSession = await startNewInteractiveSession(ownerPage, options);
    sessionId = createdSession.id;

    log(`Connecting passive recorder client to session ${sessionId}`);
    const recorderState = await connectRecorderToSession(recorderPage, options, sessionId);
    if (recorderState.clientRole !== 'recorder') {
      throw new Error(`Expected recorder page to connect with role=recorder, got ${recorderState.clientRole}`);
    }

    const statusBefore = await poll(
      `recorder session status for ${sessionId}`,
      () => fetchSessionStatus(accessToken, options, sessionId),
      (status) => status?.browser_clients >= 2 && status?.recorder_clients === 1,
      options.connectTimeoutMs,
    );

    await recorderPage.evaluate(() => {
      window.__bpaneRecording.setAutoDownload(false);
      return window.__bpaneRecording.start();
    });
    log(`Recording started for session ${sessionId}`);

    await nudgeRemotePage(ownerPage);
    await sleep(options.recordDurationMs);
    await nudgeRemotePage(ownerPage);

    const recordingStop = await recorderPage.evaluate(async () => {
      const blob = await window.__bpaneRecording.stop();
      return { size: blob?.size ?? 0, type: blob?.type ?? '' };
    });
    if (!recordingStop.size) {
      throw new Error('Recording finalized without any media bytes.');
    }

    if (!outputPath) {
      const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-recording-smoke-'));
      outputPath = path.join(tempDir, `browserpane-${sessionId}.webm`);
    }
    await fs.mkdir(path.dirname(outputPath), { recursive: true });

    const [download] = await Promise.all([
      recorderPage.waitForEvent('download'),
      recorderPage.evaluate(() => window.__bpaneRecording.downloadLast()),
    ]);
    await download.saveAs(outputPath);

    const artifactStats = await fs.stat(outputPath);
    if (artifactStats.size <= 1024) {
      throw new Error(`Recording artifact is unexpectedly small (${artifactStats.size} bytes).`);
    }

    const sessionResource = await fetchSessionResource(accessToken, options, sessionId);
    const statusAfter = await fetchSessionStatus(accessToken, options, sessionId);
    const summary = {
      scenario: 'recording-compose-smoke',
      pageUrl: options.pageUrl,
      session: {
        id: sessionId,
        runtime: sessionResource.runtime,
      },
      recording: {
        output_path: outputPath,
        bytes: artifactStats.size,
        mime_type: recordingStop.type,
        duration_ms: options.recordDurationMs,
      },
      status_before_recording: statusBefore,
      status_after_recording: statusAfter,
      recorder_client: recorderState,
    };

    log(`Recorded ${artifactStats.size} bytes to ${outputPath}`);
    console.log(JSON.stringify(summary, null, 2));

    if (options.summaryPath) {
      await fs.mkdir(path.dirname(options.summaryPath), { recursive: true });
      await fs.writeFile(options.summaryPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
    }
  } finally {
    if (recorderPage) {
      await disconnectPage(recorderPage);
    }
    if (ownerPage) {
      await disconnectPage(ownerPage);
    }
    if (accessToken && sessionId) {
      await deleteSession(accessToken, options, sessionId);
    }
    if (context) {
      await context.close().catch(() => {});
    }
    await browser.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error(`[recording-smoke] ${error instanceof Error ? error.stack ?? error.message : String(error)}`);
  process.exitCode = 1;
});
