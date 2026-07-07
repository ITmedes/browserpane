import fs from "node:fs/promises";
import path from "node:path";
import { chromium, type Browser, type BrowserContext, type Page } from "playwright-core";

type RecorderPageRuntimeOptions = {
  pageUrl: string;
  certSpki: string;
  chromeExecutablePath: string;
  connectGatewayUrl: string;
  connectTimeoutMs: number;
  headless: boolean;
};

type RecordingArtifact = {
  outputPath: string;
  bytes: number;
  mimeType: string;
  durationMs: number;
};

type RecorderConnectOptions = {
  gatewayUrl: string;
  transportPath: string;
  connectTicket: string;
};

export class RecorderPageRuntime {
  private readonly pageUrl: string;
  private readonly certSpki: string;
  private readonly chromeExecutablePath: string;
  private readonly connectGatewayUrl: string;
  private readonly connectTimeoutMs: number;
  private readonly headless: boolean;
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private page: Page | null = null;
  private startedAtMs = 0;

  constructor(options: RecorderPageRuntimeOptions) {
    this.pageUrl = options.pageUrl;
    this.certSpki = options.certSpki.trim();
    this.chromeExecutablePath = options.chromeExecutablePath;
    this.connectGatewayUrl = options.connectGatewayUrl.trim();
    this.connectTimeoutMs = options.connectTimeoutMs;
    this.headless = options.headless;
  }

  async start(options: RecorderConnectOptions): Promise<void> {
    const browser = await chromium.launch({
      headless: this.headless,
      executablePath: this.chromeExecutablePath,
      args: this.buildChromeArgs(),
    });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      deviceScaleFactor: 1,
      acceptDownloads: true,
    });
    const page = await context.newPage();

    this.browser = browser;
    this.context = context;
    this.page = page;

    await page.goto(this.buildRecorderPageUrl(), { waitUntil: "networkidle" });
    await page.waitForFunction(
      () => Boolean(window.__bpaneRecorder),
      undefined,
      { timeout: this.connectTimeoutMs },
    );
    await page.evaluate(async (connectOptions) => {
      const recorder = window.__bpaneRecorder;
      if (!recorder) {
        throw new Error("BrowserPane recorder API is not available");
      }
      await recorder.connect(connectOptions);
    }, {
      gatewayUrl: this.recorderGatewayUrl(options.gatewayUrl, options.transportPath),
      connectTicket: options.connectTicket,
    });
    await page.waitForSelector("#desktop-container canvas", { timeout: this.connectTimeoutMs });
    await this.waitForRenderableSurface(page);
    await this.waitForVisualActivity(page);
    await page.evaluate(() => {
      const recorder = window.__bpaneRecorder;
      if (!recorder) {
        throw new Error("BrowserPane recording API is not available");
      }
      return recorder.start();
    });
    this.startedAtMs = Date.now();
  }

  async waitForMinimumCapture(minDurationMs: number): Promise<void> {
    const remainingMs = this.startedAtMs + minDurationMs - Date.now();
    if (remainingMs <= 0) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, remainingMs));
  }

  async stopAndDownload(outputPath: string): Promise<RecordingArtifact> {
    const page = this.requirePage();
    const stopResult = await page.evaluate(async () => {
      const recorder = window.__bpaneRecorder;
      if (!recorder) {
        throw new Error("BrowserPane recording API is not available");
      }
      return await recorder.stop();
    });
    if (!stopResult.size) {
      throw new Error("recording finalized without any media bytes");
    }

    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    const [download] = await Promise.all([
      page.waitForEvent("download"),
      page.evaluate(() => {
        const recorder = window.__bpaneRecorder;
        if (!recorder) {
          throw new Error("BrowserPane recording API is not available");
        }
        recorder.downloadLast();
      }),
    ]);
    await download.saveAs(outputPath);
    const stats = await fs.stat(outputPath);
    return {
      outputPath,
      bytes: stats.size,
      mimeType: stopResult.type || "video/webm",
      durationMs: Math.max(0, Date.now() - this.startedAtMs),
    };
  }

  async close(): Promise<void> {
    if (this.page) {
      await this.page
        .evaluate(async () => {
          await window.__bpaneRecorder?.disconnect();
        })
        .catch(() => {});
    }
    await this.context?.close().catch(() => {});
    await this.browser?.close().catch(() => {});
    this.page = null;
    this.context = null;
    this.browser = null;
  }

  private buildChromeArgs(): string[] {
    const args = [
      `--origin-to-force-quic-on=${this.quicOrigin()}`,
      `--unsafely-treat-insecure-origin-as-secure=${this.pageOrigin()}`,
      "--no-sandbox",
      "--disable-dev-shm-usage",
      "--disable-background-timer-throttling",
      "--disable-renderer-backgrounding",
      "--disable-backgrounding-occluded-windows",
    ];
    if (this.certSpki) {
      args.push(`--ignore-certificate-errors-spki-list=${this.certSpki}`);
    }
    return args;
  }

  private async waitForRenderableSurface(page: Page): Promise<void> {
    await page.waitForFunction(
      () => {
        const canvas = document.querySelector("#desktop-container canvas") as HTMLCanvasElement | null;
        return Boolean(canvas && canvas.width > 1 && canvas.height > 1);
      },
      undefined,
      { timeout: this.connectTimeoutMs, polling: 100 },
    );
  }

  private async waitForVisualActivity(page: Page): Promise<void> {
    await page.waitForFunction(
      () => {
        const stats = window.__bpaneRecorder?.getStats?.();
        const frameCount = typeof stats?.frameCount === "number" ? stats.frameCount : 0;
        const renderedTileUpdates =
          typeof stats?.renderedTileUpdates === "number" ? stats.renderedTileUpdates : 0;
        return frameCount > 0 || renderedTileUpdates > 0;
      },
      undefined,
      { timeout: Math.min(this.connectTimeoutMs, 5_000), polling: 100 },
    ).catch(() => {});
  }

  private quicOrigin(): string {
    if (!this.connectGatewayUrl) {
      return "localhost:4433";
    }
    try {
      return new URL(this.connectGatewayUrl).host;
    } catch {
      return "localhost:4433";
    }
  }

  private pageOrigin(): string {
    try {
      return new URL(this.pageUrl).origin;
    } catch {
      return "http://localhost:8080";
    }
  }

  private buildRecorderPageUrl(): string {
    const url = new URL(this.pageUrl);
    return url.toString();
  }

  private recorderGatewayUrl(gatewayUrl: string, transportPath: string): string {
    const baseUrl = this.connectGatewayUrl || gatewayUrl;
    return `${baseUrl.replace(/\/+$/, "")}${transportPath}`;
  }

  private requirePage(): Page {
    if (!this.page) {
      throw new Error("recorder page is not active");
    }
    return this.page;
  }
}
