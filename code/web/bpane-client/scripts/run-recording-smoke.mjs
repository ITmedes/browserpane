import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { chromium } from 'playwright-core';
import { testEmbedPageUrl } from './workflow-smoke-lib.mjs';

const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  metricsUrl: process.env.BPANE_METRICS_URL ?? 'http://localhost:8932/metrics',
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
    } else if (arg === '--metrics-url' && next) {
      options.metricsUrl = next;
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
  --metrics-url <url>         Gateway OpenMetrics URL (default: ${DEFAULTS.metricsUrl})
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

function runBpaneCli(args, accessToken, options) {
  const result = spawnSync(
    process.execPath,
    [path.join(process.cwd(), 'scripts', 'bpane-cli.mjs'), ...args],
    {
      cwd: process.cwd(),
      env: {
        ...process.env,
        BPANE_ACCESS_TOKEN: accessToken,
        BPANE_BASE_URL: options.pageUrl,
      },
      encoding: 'utf8',
    },
  );
  if (result.status !== 0) {
    throw new Error(
      `bpane CLI failed with code ${result.status ?? 'unknown'}: ${result.stderr || result.stdout}`,
    );
  }
  return result.stdout.trim() ? JSON.parse(result.stdout) : null;
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

async function configurePage(page, options, pageUrl = testEmbedPageUrl(options)) {
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

function buildBrowserOnlyPageUrl(pageUrl) {
  const url = new URL(testEmbedPageUrl({ pageUrl }));
  url.searchParams.set('layout', 'browser-only');
  return url.toString();
}

function buildRecorderPageUrl(pageUrl) {
  const url = new URL(buildBrowserOnlyPageUrl(pageUrl));
  url.searchParams.set('client_role', 'recorder');
  return url.toString();
}

async function connectInteractiveToSession(page, options, sessionId) {
  await page.evaluate(async (id) => {
    await window.__bpaneControl.refreshSessions({ preserveSelection: true, silent: true });
    await window.__bpaneControl.selectSession(id);
    await window.__bpaneControl.connectSelected({ clientRole: 'interactive' });
  }, sessionId);
  await page.waitForFunction(
    () => window.__bpaneControl?.getState?.()?.connected === true,
    { timeout: options.connectTimeoutMs },
  );
  await page.waitForSelector('#desktop-container canvas', { timeout: options.connectTimeoutMs });
  return await page.evaluate(() => window.__bpaneControl.getState());
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
  await Promise.race([
    page.evaluate(async () => {
      const state = window.__bpaneControl?.getState?.();
      if (state?.connected) {
        await window.__bpaneControl.disconnect();
      }
    }).catch(() => {}),
    sleep(5000),
  ]);
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

async function fetchOpenMetrics(options) {
  const response = await fetch(options.metricsUrl);
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`Metrics HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  return await response.text();
}

function openMetricsInteger(body, metricName) {
  const prefix = `${metricName} `;
  const sample = body
    .split('\n')
    .find((line) => line.startsWith(prefix))
    ?.slice(prefix.length);
  if (!sample || !/^\d+$/.test(sample)) {
    throw new Error(`OpenMetrics response is missing integer counter ${metricName}`);
  }
  return Number(sample);
}

function assertCounterAdvanced(before, after, metricName) {
  const beforeValue = openMetricsInteger(before, metricName);
  const afterValue = openMetricsInteger(after, metricName);
  if (afterValue <= beforeValue) {
    throw new Error(
      `Expected ${metricName} to advance, got ${beforeValue} -> ${afterValue}`,
    );
  }
  return { before: beforeValue, after: afterValue, delta: afterValue - beforeValue };
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

async function createSessionResource(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      owner_mode: 'collaborative',
      idle_timeout_sec: 300,
      recording: {
        mode: 'always',
        format: 'webm',
      },
      integration_context: {
        source: 'run-recording-smoke',
        origin: new URL(options.pageUrl).origin,
      },
    }),
  });
}

async function fetchSessionPlayback(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/recording-playback`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchSessionPlaybackManifest(accessToken, options, sessionId) {
  return await fetchJson(
    `${options.pageUrl}/api/v1/sessions/${sessionId}/recording-playback/manifest`,
    {
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
}

async function fetchSessionPlaybackExport(accessToken, options, sessionId) {
  const response = await fetch(
    `${options.pageUrl}/api/v1/sessions/${sessionId}/recording-playback/export`,
    {
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
  if (!response.ok) {
    const detail = await response.text().catch(() => '');
    throw new Error(`HTTP ${response.status}${detail ? ` ${detail}` : ''}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  return {
    bytes: bytes.byteLength,
    contentType: response.headers.get('content-type') ?? '',
  };
}

async function fetchRecordingOperations(accessToken, options) {
  return await fetchJson(`${options.pageUrl}/api/v1/recording/operations`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function fetchSessionRecordings(accessToken, options, sessionId) {
  return await fetchJson(`${options.pageUrl}/api/v1/sessions/${sessionId}/recordings`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

async function disconnectAllSessionConnections(accessToken, options, sessionId) {
  return await fetchJson(
    `${options.pageUrl}/api/v1/sessions/${sessionId}/connections/disconnect-all`,
    {
      method: 'POST',
      headers: { Authorization: `Bearer ${accessToken}` },
    },
  );
}

function resolveSegmentOutputPath(requestedOutputPath, tempDir, sessionId, segmentIndex) {
  if (!requestedOutputPath) {
    return path.join(tempDir, `browserpane-${sessionId}-segment-${segmentIndex}.webm`);
  }
  if (segmentIndex === 1) {
    return requestedOutputPath;
  }
  const extension = path.extname(requestedOutputPath) || '.webm';
  const base = requestedOutputPath.slice(0, requestedOutputPath.length - extension.length);
  return `${base}-segment-${segmentIndex}${extension}`;
}

async function savePlaywrightDownload(download, targetPath) {
  await fs.mkdir(path.dirname(targetPath), { recursive: true });
  await download.saveAs(targetPath);
  const stats = await fs.stat(targetPath);
  return {
    path: targetPath,
    bytes: stats.size,
    suggestedFilename: download.suggestedFilename(),
  };
}

async function captureLocalRecordingSegment({
  ownerPage,
  recorderPage,
  options,
  sessionId,
  requestedOutputPath,
  tempDir,
}) {
  await recorderPage.evaluate(() => {
    window.__bpaneRecording.setAutoDownload(false);
    return window.__bpaneRecording.start();
  });
  log(`Browser-local recording started for session ${sessionId}`);
  await nudgeRemotePage(ownerPage);
  await sleep(options.recordDurationMs);
  await nudgeRemotePage(ownerPage);

  const recordingStop = await recorderPage.evaluate(async () => {
    const blob = await window.__bpaneRecording.stop();
    return { size: blob?.size ?? 0, type: blob?.type ?? '' };
  });
  if (!recordingStop.size) {
    throw new Error('Browser-local recording finalized without any media bytes.');
  }
  const outputPath = resolveSegmentOutputPath(
    requestedOutputPath,
    tempDir,
    sessionId,
    1,
  );
  const [download] = await Promise.all([
    recorderPage.waitForEvent('download'),
    recorderPage.evaluate(() => window.__bpaneRecording.downloadLast()),
  ]);
  const saved = await savePlaywrightDownload(download, outputPath);
  if (saved.bytes <= 1024) {
    throw new Error(`Browser-local recording is unexpectedly small (${saved.bytes} bytes).`);
  }
  return {
    ...saved,
    mimeType: recordingStop.type || 'video/webm',
    durationMs: options.recordDurationMs,
  };
}

async function downloadRecordingFromLibrary(page, recordingId, targetPath) {
  const button = page.locator(
    `[data-action="download-recording"][data-recording-id="${recordingId}"]`,
  );
  await button.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    button.click(),
  ]);
  return await savePlaywrightDownload(download, targetPath);
}

async function downloadPlaybackExportFromLibrary(page, targetPath) {
  const button = page.locator('#btn-recording-playback-download');
  await button.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    button.click(),
  ]);
  return await savePlaywrightDownload(download, targetPath);
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
  let tempDir = '';

  try {
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
      acceptDownloads: true,
    });

    ownerPage = await context.newPage();
    await configurePage(ownerPage, options);
    await ensureLoggedIn(ownerPage, options);
    accessToken = (await getAccessToken(ownerPage)) ?? '';
    if (!accessToken) {
      throw new Error('Failed to acquire an access token from the owner page.');
    }
    const metricsBefore = await fetchOpenMetrics(options);

    recorderPage = await context.newPage();
    await configurePage(recorderPage, options);
    await ensureLoggedIn(recorderPage, options);
    await configurePage(recorderPage, options, buildRecorderPageUrl(options.pageUrl));
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'bpane-recording-smoke-'));

    log('Starting source session');
    const createdSession = await createSessionResource(accessToken, options);
    sessionId = createdSession.id;
    await connectInteractiveToSession(ownerPage, options, sessionId);

    log(`Connecting passive recorder client to session ${sessionId}`);
    const recorderState = await connectRecorderToSession(recorderPage, options, sessionId);
    if (recorderState.clientRole !== 'recorder') {
      throw new Error(`Expected recorder page to connect with role=recorder, got ${recorderState.clientRole}`);
    }

    const statusBefore = await poll(
      `recorder session status for ${sessionId}`,
      () => fetchSessionStatus(accessToken, options, sessionId),
      (status) => status?.browser_clients >= 3 && status?.recorder_clients >= 2,
      options.connectTimeoutMs,
    );
    const localRecording = await captureLocalRecordingSegment({
      ownerPage,
      recorderPage,
      options,
      sessionId,
      requestedOutputPath: options.outputPath,
      tempDir,
    });
    await disconnectAllSessionConnections(accessToken, options, sessionId);
    const recordingCatalogResponse = await poll(
      `retained recorder-worker segment for ${sessionId}`,
      () => fetchSessionRecordings(accessToken, options, sessionId),
      (catalog) => catalog?.recordings?.some((recording) => (
        recording.state === 'failed'
        || (recording.state === 'ready' && recording.artifact_available === true)
      )),
      options.connectTimeoutMs * 2,
    );
    const failedRecording = recordingCatalogResponse.recordings.find(
      (recording) => recording.state === 'failed',
    );
    if (failedRecording) {
      throw new Error(`Recorder-worker recording failed: ${failedRecording.error ?? 'unknown error'}`);
    }
    const recordingSegments = recordingCatalogResponse.recordings
      .filter((recording) => recording.state === 'ready' && recording.artifact_available === true)
      .map((recording, index) => ({
        index: index + 1,
        id: recording.id,
        outputPath: null,
        bytes: recording.bytes,
        mimeType: recording.mime_type || 'video/webm',
        durationMs: recording.duration_ms,
        suggestedFilename: `${recording.id}.webm`,
        controlPlane: recording,
      }));
    if (!recordingSegments.length || recordingSegments.some((segment) => segment.bytes <= 1024)) {
      throw new Error('Recorder-worker did not retain a non-empty WebM segment.');
    }

    const playback = await fetchSessionPlayback(accessToken, options, sessionId);
    const playbackManifest = await fetchSessionPlaybackManifest(accessToken, options, sessionId);
    const playbackExport = await fetchSessionPlaybackExport(accessToken, options, sessionId);
    const recordingOperations = await fetchRecordingOperations(accessToken, options);

    if (playback.state !== 'ready') {
      throw new Error(`Expected playback state=ready, got ${playback.state}`);
    }
    if (playback.included_segment_count !== recordingSegments.length) {
      throw new Error(
        `Expected playback to include exactly ${recordingSegments.length} segments, got ${playback.included_segment_count}.`,
      );
    }
    if (playbackManifest.segments?.length !== recordingSegments.length) {
      throw new Error('Playback manifest did not include the expected segment entries.');
    }
    const largestSegmentBytes = Math.max(...recordingSegments.map((segment) => segment.bytes));
    if (playbackExport.contentType !== 'application/zip' || playbackExport.bytes <= largestSegmentBytes) {
      throw new Error('Playback export bundle was not generated as a non-empty zip artifact.');
    }

    const cliRecordingList = runBpaneCli(
      ['session', 'recording', 'list', sessionId],
      accessToken,
      options,
    );
    if (
      !Array.isArray(cliRecordingList.recordings)
      || !recordingSegments.every((segment) => (
        cliRecordingList.recordings.some((recording) => recording.id === segment.id)
      ))
    ) {
      throw new Error('CLI recording list did not include every completed segment.');
    }
    const cliSegment = recordingSegments[0];
    const cliRecording = runBpaneCli(
      ['session', 'recording', 'get', sessionId, cliSegment.id],
      accessToken,
      options,
    );
    if (
      cliRecording.id !== cliSegment.id
      || cliRecording.state !== 'ready'
      || cliRecording.artifact_available !== true
    ) {
      throw new Error(`CLI recording get returned unexpected data: ${JSON.stringify(cliRecording)}`);
    }
    const cliRecordingPath = path.join(tempDir, 'cli-downloads', 'segment.webm');
    const cliRecordingDownload = runBpaneCli(
      [
        'session',
        'recording',
        'download',
        sessionId,
        cliSegment.id,
        '--output',
        cliRecordingPath,
      ],
      accessToken,
      options,
    );
    if (
      cliRecordingDownload.byte_count !== cliSegment.bytes
      || (await fs.stat(cliRecordingPath)).size !== cliSegment.bytes
      || cliRecordingDownload.content_type !== cliSegment.mimeType
    ) {
      throw new Error('CLI recording download did not preserve the completed WebM segment.');
    }
    const cliPlayback = runBpaneCli(
      ['session', 'playback', 'get', sessionId],
      accessToken,
      options,
    );
    const cliPlaybackManifest = runBpaneCli(
      ['session', 'playback', 'manifest', sessionId],
      accessToken,
      options,
    );
    if (
      cliPlayback.state !== 'ready'
      || cliPlaybackManifest.segments?.length !== recordingSegments.length
    ) {
      throw new Error('CLI playback inspection did not expose every completed segment.');
    }
    const cliPlaybackPath = path.join(tempDir, 'cli-downloads', 'playback.zip');
    const cliPlaybackExport = runBpaneCli(
      [
        'session',
        'playback',
        'export',
        sessionId,
        '--output',
        cliPlaybackPath,
      ],
      accessToken,
      options,
    );
    if (
      cliPlaybackExport.byte_count !== playbackExport.bytes
      || (await fs.stat(cliPlaybackPath)).size !== playbackExport.bytes
      || cliPlaybackExport.content_type !== 'application/zip'
    ) {
      throw new Error('CLI playback export did not preserve the generated zip bundle.');
    }

    const refreshButton = ownerPage.locator('#btn-recording-library-refresh');
    await refreshButton.waitFor({ state: 'visible', timeout: options.connectTimeoutMs });
    await refreshButton.click();
    const recordingCatalog = await poll(
      `recording library for ${sessionId}`,
      () => ownerPage.evaluate(() => window.__bpaneRecording.getCatalogState()),
      (state) => (
        state?.loaded === true
        && state?.recordings?.length >= recordingSegments.length
        && state?.playback?.included_segment_count >= recordingSegments.length
      ),
      options.connectTimeoutMs,
    );

    const uiDownloadDir = path.join(tempDir, 'ui-downloads');
    const downloadedSegments = [];
    for (const segment of recordingSegments) {
      const targetPath = path.join(uiDownloadDir, `segment-${segment.index}.webm`);
      const downloaded = await downloadRecordingFromLibrary(ownerPage, segment.id, targetPath);
      if (!downloaded.suggestedFilename.endsWith('.webm')) {
        throw new Error(`Expected segment download filename to end with .webm, got ${downloaded.suggestedFilename}`);
      }
      if (downloaded.bytes !== segment.bytes) {
        throw new Error(
          `Segment download size mismatch for ${segment.id}: expected ${segment.bytes}, got ${downloaded.bytes}.`,
        );
      }
      downloadedSegments.push({
        recording_id: segment.id,
        path: downloaded.path,
        bytes: downloaded.bytes,
        suggested_filename: downloaded.suggestedFilename,
      });
    }

    const downloadedPlaybackExport = await downloadPlaybackExportFromLibrary(
      ownerPage,
      path.join(uiDownloadDir, 'recording-playback.zip'),
    );
    if (!downloadedPlaybackExport.suggestedFilename.endsWith('.zip')) {
      throw new Error(
        `Expected playback export download filename to end with .zip, got ${downloadedPlaybackExport.suggestedFilename}`,
      );
    }
    if (downloadedPlaybackExport.bytes !== playbackExport.bytes) {
      throw new Error(
        `Playback export download size mismatch: expected ${playbackExport.bytes}, got ${downloadedPlaybackExport.bytes}.`,
      );
    }

    const sessionResource = await fetchSessionResource(accessToken, options, sessionId);
    const statusAfter = await fetchSessionStatus(accessToken, options, sessionId);
    const metricsAfter = await fetchOpenMetrics(options);
    const recordingMetrics = Object.fromEntries(
      [
        'browserpane_gateway_recording_artifact_finalize_requests_total',
        'browserpane_gateway_recording_artifact_finalize_successes_total',
        'browserpane_gateway_recording_playback_manifest_requests_total',
        'browserpane_gateway_recording_playback_export_requests_total',
        'browserpane_gateway_recording_playback_export_successes_total',
        'browserpane_gateway_recording_playback_export_bytes_total',
      ].map((metricName) => [
        metricName,
        assertCounterAdvanced(metricsBefore, metricsAfter, metricName),
      ]),
    );
    for (const forbidden of [
      accessToken,
      sessionId,
      ...recordingSegments.map((segment) => segment.id),
    ]) {
      if (forbidden && metricsAfter.includes(forbidden)) {
        throw new Error('Recording OpenMetrics response exposed a forbidden resource value.');
      }
    }
    const summary = {
      scenario: 'recording-compose-smoke',
      pageUrl: options.pageUrl,
      session: {
        id: sessionId,
        runtime: sessionResource.runtime,
      },
      recordings: recordingSegments.map((segment) => ({
        id: segment.id,
        output_path: segment.outputPath,
        bytes: segment.bytes,
        mime_type: segment.mimeType,
        duration_ms: segment.durationMs,
        suggested_filename: segment.suggestedFilename,
        control_plane: segment.controlPlane,
      })),
      browser_local_recording: localRecording,
      playback,
      playback_manifest: playbackManifest,
      playback_export: playbackExport,
      cli_recording_evidence: {
        recording: cliRecording,
        recording_download: cliRecordingDownload,
        playback: cliPlayback,
        playback_manifest: cliPlaybackManifest,
        playback_export: cliPlaybackExport,
      },
      ui_recording_library: {
        recordings: recordingCatalog.recordings,
        playback: recordingCatalog.playback,
        downloaded_segments: downloadedSegments,
        downloaded_playback_export: {
          path: downloadedPlaybackExport.path,
          bytes: downloadedPlaybackExport.bytes,
          suggested_filename: downloadedPlaybackExport.suggestedFilename,
        },
      },
      recording_operations: recordingOperations,
      recording_metrics: recordingMetrics,
      status_before_recording: statusBefore,
      status_after_recording: statusAfter,
      recorder_client: recorderState,
    };

    log(
      `Recorded ${recordingSegments.length} segments for session ${sessionId} and verified UI downloads from test-embed.`,
    );
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
