import fs from 'node:fs/promises';
import { execFile } from 'node:child_process';
import process from 'node:process';
import { promisify } from 'node:util';
import { chromium } from 'playwright-core';
import { testEmbedPageUrl } from './workflow-smoke-lib.mjs';

const execFileAsync = promisify(execFile);

const PROFILE_PRESETS = {
  stress: {
    remoteUrl: 'http://web:8080/benchmark-scroll.html',
    cycles: 4,
    downSteps: 18,
    upSteps: 18,
    wheelDeltaY: 720,
    stepDelayMs: 110,
    settleMs: 1200,
  },
  realistic: {
    remoteUrl: 'http://web:8080/benchmark-article.html',
    cycles: 3,
    downSteps: 16,
    upSteps: 16,
    wheelDeltaY: 360,
    stepDelayMs: 140,
    settleMs: 1400,
  },
};

const DEFAULTS = {
  profile: '',
  pageUrl: 'http://localhost:8080',
  gatewayStatusUrl: 'http://localhost:8932/api/session/status',
  hostCdpUrl: process.env.BPANE_BENCHMARK_HOST_CDP ?? '',
  certSpki: process.env.BPANE_BENCHMARK_CERT_SPKI ?? '',
  remoteUrl: '',
  hostWindowWidth: 1600,
  hostWindowHeight: 1000,
  renderBackend: 'auto',
  scrollCopy: true,
  hiDpi: true,
  headless: false,
  connectTimeoutMs: 30000,
  remoteSettleMs: 800,
  settleMs: 1200,
  stepDelayMs: 110,
  downSteps: 18,
  upSteps: 18,
  cycles: 4,
  wheelDeltaY: 720,
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
    if (arg === '--profile' && next) {
      applyProfile(options, next);
      i++;
    } else if (arg === '--page-url' && next) {
      options.pageUrl = next;
      i++;
    } else if (arg === '--gateway-status-url' && next) {
      options.gatewayStatusUrl = next;
      i++;
    } else if (arg === '--host-cdp-url' && next) {
      options.hostCdpUrl = next;
      i++;
    } else if (arg === '--cert-spki' && next) {
      options.certSpki = next;
      i++;
    } else if (arg === '--remote-url' && next) {
      options.remoteUrl = next;
      i++;
    } else if (arg === '--host-window-width' && next) {
      options.hostWindowWidth = Number(next);
      i++;
    } else if (arg === '--host-window-height' && next) {
      options.hostWindowHeight = Number(next);
      i++;
    } else if (arg === '--render-backend' && next) {
      options.renderBackend = next;
      i++;
    } else if (arg === '--scroll-copy' && next) {
      options.scrollCopy = next !== 'off';
      i++;
    } else if (arg === '--hidpi' && next) {
      options.hiDpi = next !== 'off';
      i++;
    } else if (arg === '--headless') {
      options.headless = true;
    } else if (arg === '--connect-timeout-ms' && next) {
      options.connectTimeoutMs = Number(next);
      i++;
    } else if (arg === '--remote-settle-ms' && next) {
      options.remoteSettleMs = Number(next);
      i++;
    } else if (arg === '--settle-ms' && next) {
      options.settleMs = Number(next);
      i++;
    } else if (arg === '--step-delay-ms' && next) {
      options.stepDelayMs = Number(next);
      i++;
    } else if (arg === '--down-steps' && next) {
      options.downSteps = Number(next);
      i++;
    } else if (arg === '--up-steps' && next) {
      options.upSteps = Number(next);
      i++;
    } else if (arg === '--cycles' && next) {
      options.cycles = Number(next);
      i++;
    } else if (arg === '--wheel-delta-y' && next) {
      options.wheelDeltaY = Number(next);
      i++;
    } else if (arg === '--output' && next) {
      options.outputPath = next;
      i++;
    } else if (arg === '--help') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return options;
}

function applyProfile(options, profileName) {
  const preset = PROFILE_PRESETS[profileName];
  if (!preset) {
    throw new Error(`Unknown benchmark profile: ${profileName}`);
  }
  options.profile = profileName;
  Object.assign(options, preset);
}

function printHelp() {
  console.log(`
Usage: node scripts/run-scroll-benchmark.mjs [options]

Options:
  --profile <name>            stress | realistic
  --page-url <url>            Local dev page URL (default: ${DEFAULTS.pageUrl})
  --gateway-status-url <url>  Gateway status API URL (default: ${DEFAULTS.gatewayStatusUrl})
  --host-cdp-url <url>        Host Chromium CDP endpoint
  --cert-spki <base64>        SPKI pin for the local gateway cert
  --remote-url <url>          Remote page URL to open in host Chromium before the run
  --host-window-width <px>    Host Chromium window width (default: ${DEFAULTS.hostWindowWidth})
  --host-window-height <px>   Host Chromium window height (default: ${DEFAULTS.hostWindowHeight})
  --render-backend <mode>     auto | webgl2 | canvas2d
  --scroll-copy <on|off>      Toggle scroll-copy (default: on)
  --hidpi <on|off>            Toggle HiDPI (default: on)
  --remote-settle-ms <ms>     Delay after remote navigation (default: ${DEFAULTS.remoteSettleMs})
  --cycles <n>                Down/up scroll cycles (default: ${DEFAULTS.cycles})
  --down-steps <n>            Wheel steps per down cycle (default: ${DEFAULTS.downSteps})
  --up-steps <n>              Wheel steps per up cycle (default: ${DEFAULTS.upSteps})
  --wheel-delta-y <n>         Wheel delta per step (default: ${DEFAULTS.wheelDeltaY})
  --step-delay-ms <ms>        Delay between wheel steps (default: ${DEFAULTS.stepDelayMs})
  --settle-ms <ms>            Settle delay before/after sample (default: ${DEFAULTS.settleMs})
  --connect-timeout-ms <ms>   Connect timeout (default: ${DEFAULTS.connectTimeoutMs})
  --output <path>             Write JSON summary to file
  --headless                  Run headless

Environment:
  BPANE_BENCHMARK_CHROME      Explicit Chrome/Chromium executable path
  BPANE_BENCHMARK_HOST_CDP    Default host Chromium CDP endpoint
  BPANE_BENCHMARK_CERT_SPKI   Default SPKI pin for the local gateway cert

Notes:
  Profiles:
    stress     -> remote ${PROFILE_PRESETS.stress.remoteUrl}
    realistic  -> remote ${PROFILE_PRESETS.realistic.remoteUrl}
`);
}

async function resolveChromeExecutable() {
  for (const path of COMMON_CHROME_PATHS) {
    try {
      await fs.access(path);
      return path;
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

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function fetchGatewayStatus(options, accessToken = '') {
  if (!options.gatewayStatusUrl) {
    return null;
  }
  const headers = accessToken
    ? { Authorization: `Bearer ${accessToken}` }
    : {};
  try {
    const response = await fetch(options.gatewayStatusUrl, { headers });
    if (!response.ok) {
      throw new Error(`status ${response.status}`);
    }
    return await response.json();
  } catch (error) {
    const fallbackUrl = await resolveGatewayContainerStatusUrl(options.gatewayStatusUrl);
    if (fallbackUrl) {
      try {
        const response = await fetch(fallbackUrl, { headers });
        if (!response.ok) {
          throw new Error(`status ${response.status}`);
        }
        return {
          ...(await response.json()),
          resolvedFromContainer: true,
          resolvedUrl: fallbackUrl,
        };
      } catch (fallbackError) {
        return {
          error: String(fallbackError),
          url: fallbackUrl,
          initialError: String(error),
        };
      }
    }
    return {
      error: String(error),
      url: options.gatewayStatusUrl,
    };
  }
}

async function resolveGatewayContainerStatusUrl(url) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  if (!['localhost', '127.0.0.1'].includes(parsed.hostname)) {
    return null;
  }
  try {
    const { stdout } = await execFileAsync(
      'docker',
      ['inspect', '-f', '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}', 'deploy-gateway-1'],
      { timeout: 3000 },
    );
    const ip = stdout.trim();
    if (!ip) {
      return null;
    }
    parsed.hostname = ip;
    return parsed.toString();
  } catch {
    return null;
  }
}

async function setHostWindowBounds(page, options) {
  const session = await page.context().newCDPSession(page);
  try {
    const { windowId } = await session.send('Browser.getWindowForTarget');
    await session.send('Browser.setWindowBounds', {
      windowId,
      bounds: {
        windowState: 'normal',
        width: options.hostWindowWidth,
        height: options.hostWindowHeight,
      },
    });
  } finally {
    await session.detach().catch(() => {});
  }
}

async function setRemotePage(options) {
  if (!options.hostCdpUrl || !options.remoteUrl) {
    return null;
  }
  const remoteBrowser = await chromium.connectOverCDP(options.hostCdpUrl);
  try {
    const context = remoteBrowser.contexts()[0] ?? await remoteBrowser.newContext();
    let page = context.pages().find((candidate) => candidate.url().startsWith(options.remoteUrl));
    if (!page) {
      page = await context.newPage();
    }
    await setHostWindowBounds(page, options);
    await page.goto(options.remoteUrl, { waitUntil: 'load' });
    await page.bringToFront();
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.waitForTimeout(options.remoteSettleMs);
    return {
      title: await page.title(),
      url: page.url(),
    };
  } finally {
    await remoteBrowser.close();
  }
}

async function configurePage(page, options) {
  await page.goto(testEmbedPageUrl(options), { waitUntil: 'networkidle' });
  await page.waitForFunction(() => Boolean(window.__bpaneBenchmarkMetrics));
  await page.selectOption('#render-backend-select', options.renderBackend);
  await page.locator('#scroll-copy-toggle').setChecked(options.scrollCopy);
  await page.locator('#hidpi-toggle').setChecked(options.hiDpi);
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

async function ensureLoggedIn(page, options) {
  const authConfig = await fetchAuthConfig(options);
  if (!authConfig || authConfig.mode !== 'oidc') {
    return null;
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
    throw new Error('OIDC auth is enabled, but no example user is configured for benchmark login.');
  }

  await page.click('#btn-login');
  await page.waitForURL(/openid-connect|keycloak|realms/i, { timeout: options.connectTimeoutMs });
  await page.locator('input[name="username"], #username').fill(exampleUser.username);
  await page.locator('input[name="password"], #password').fill(exampleUser.password);
  await page.locator('input[type="submit"], #kc-login').click();
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

async function connectSession(page, options) {
  await page.click('#btn-connect');
  await page.waitForFunction(
    () => document.querySelector('#status')?.textContent?.trim() === 'Connected',
    { timeout: options.connectTimeoutMs },
  );
  await page.waitForSelector('#desktop-container canvas', { timeout: options.connectTimeoutMs });
  await page.waitForFunction(
    () => document.querySelector('#resolution')?.textContent?.includes('x'),
    { timeout: options.connectTimeoutMs },
  );
}

async function runScrollSequence(page, options) {
  const canvas = page.locator('#desktop-container canvas').first();
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error('Desktop canvas is not visible for benchmarking.');
  }

  const centerX = box.x + box.width / 2;
  const centerY = box.y + Math.min(box.height / 2, 220);
  await page.mouse.move(centerX, centerY);
  await page.mouse.click(centerX, centerY);

  for (let cycle = 0; cycle < options.cycles; cycle++) {
    for (let step = 0; step < options.downSteps; step++) {
      await page.mouse.wheel(0, options.wheelDeltaY);
      await sleep(options.stepDelayMs);
    }
    await sleep(Math.min(options.stepDelayMs * 2, 240));
    for (let step = 0; step < options.upSteps; step++) {
      await page.mouse.wheel(0, -options.wheelDeltaY);
      await sleep(options.stepDelayMs);
    }
    await sleep(Math.min(options.stepDelayMs * 2, 240));
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const executablePath = await resolveChromeExecutable();
  const certSpki = await resolveCertSpki(options);
  const remotePage = await setRemotePage(options);
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

  let page;
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
    });
    page = await context.newPage();
    page.on('console', (msg) => {
      const text = msg.text();
      if (text.includes('[bpane:error]')) {
        console.error(text);
      }
    });

    await configurePage(page, options);
    await ensureLoggedIn(page, options);
    await connectSession(page, options);
    await sleep(options.settleMs);

    await page.evaluate(() => {
      window.__bpaneBenchmarkMetrics.resetSample();
      window.__bpaneBenchmarkMetrics.startSample();
    });

    await runScrollSequence(page, options);
    await sleep(options.settleMs);

    const summary = await page.evaluate(() => {
      window.__bpaneBenchmarkMetrics.stopSample();
      return window.__bpaneBenchmarkMetrics.getSummary();
    });
    const gatewayAccessToken = await page.evaluate(
      async () => await window.__bpaneAuth?.getAccessToken?.(),
    );
    const gatewayStatus = await fetchGatewayStatus(options, gatewayAccessToken ?? '');

    if (!summary) {
      throw new Error('Benchmark summary was empty.');
    }

    const result = {
      capturedAt: new Date().toISOString(),
      config: {
        profile: options.profile || 'custom',
        pageUrl: options.pageUrl,
        gatewayStatusUrl: options.gatewayStatusUrl || null,
        hostCdpUrl: options.hostCdpUrl || null,
        certSpki: certSpki || null,
        remoteUrl: options.remoteUrl || null,
        hostWindowWidth: options.hostWindowWidth,
        hostWindowHeight: options.hostWindowHeight,
        remoteSettleMs: options.remoteSettleMs,
        renderBackend: options.renderBackend,
        scrollCopy: options.scrollCopy,
        hiDpi: options.hiDpi,
        cycles: options.cycles,
        downSteps: options.downSteps,
        upSteps: options.upSteps,
        wheelDeltaY: options.wheelDeltaY,
        stepDelayMs: options.stepDelayMs,
        settleMs: options.settleMs,
        headless: options.headless,
      },
      remotePage,
      summary,
      gatewayStatus,
    };

    const output = JSON.stringify(result, null, 2);
    console.log(output);
    if (options.outputPath) {
      await fs.writeFile(options.outputPath, output);
    }
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
