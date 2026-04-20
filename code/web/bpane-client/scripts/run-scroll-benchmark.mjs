import fs from 'node:fs/promises';
import process from 'node:process';
import { chromium } from 'playwright-core';

const DEFAULTS = {
  pageUrl: 'http://localhost:8080',
  renderBackend: 'auto',
  scrollCopy: true,
  hiDpi: true,
  headless: false,
  connectTimeoutMs: 30000,
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
    if (arg === '--page-url' && next) {
      options.pageUrl = next;
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

function printHelp() {
  console.log(`
Usage: node scripts/run-scroll-benchmark.mjs [options]

Options:
  --page-url <url>            Local dev page URL (default: ${DEFAULTS.pageUrl})
  --render-backend <mode>     auto | webgl2 | canvas2d
  --scroll-copy <on|off>      Toggle scroll-copy (default: on)
  --hidpi <on|off>            Toggle HiDPI (default: on)
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

Notes:
  For deterministic remote content, start the host browser with:
    BPANE_URL=http://web:8080/benchmark-scroll.html
  inside the compose network.
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

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function configurePage(page, options) {
  await page.goto(options.pageUrl, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => Boolean(window.__bpaneBenchmarkMetrics));
  await page.selectOption('#render-backend-select', options.renderBackend);
  await page.locator('#scroll-copy-toggle').setChecked(options.scrollCopy);
  await page.locator('#hidpi-toggle').setChecked(options.hiDpi);
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
  const browser = await chromium.launch({
    headless: options.headless,
    executablePath,
    args: [
      '--origin-to-force-quic-on=localhost:4433',
      '--disable-background-timer-throttling',
      '--disable-renderer-backgrounding',
      '--disable-backgrounding-occluded-windows',
    ],
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

    if (!summary) {
      throw new Error('Benchmark summary was empty.');
    }

    const result = {
      capturedAt: new Date().toISOString(),
      config: {
        pageUrl: options.pageUrl,
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
      summary,
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
