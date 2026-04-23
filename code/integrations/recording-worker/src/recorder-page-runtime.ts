import fs from "node:fs/promises";
import path from "node:path";
import { chromium, type Browser, type BrowserContext, type Page } from "playwright-core";

type RecorderPageRuntimeOptions = {
  pageUrl: string;
  certSpki: string;
  chromeExecutablePath: string;
  connectTimeoutMs: number;
  headless: boolean;
};

type RecordingArtifact = {
  outputPath: string;
  bytes: number;
  mimeType: string;
  durationMs: number;
};

export class RecorderPageRuntime {
  private readonly pageUrl: string;
  private readonly certSpki: string;
  private readonly chromeExecutablePath: string;
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
    this.connectTimeoutMs = options.connectTimeoutMs;
    this.headless = options.headless;
  }

  async start(sessionId: string): Promise<void> {
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

    await page.goto(this.pageUrl, { waitUntil: "networkidle" });
    await page.waitForFunction(
      () => Boolean(window.__bpaneAuth && window.__bpaneControl && window.__bpaneRecording),
      { timeout: this.connectTimeoutMs },
    );
    await this.ensureLoggedIn(page);
    await page.goto(this.buildRecorderPageUrl(), { waitUntil: "networkidle" });
    await page.waitForFunction(
      () => Boolean(window.__bpaneAuth && window.__bpaneControl && window.__bpaneRecording),
      { timeout: this.connectTimeoutMs },
    );
    await page.evaluate(
      async (selectedSessionId) => {
        const control = window.__bpaneControl;
        if (!control) {
          throw new Error("BrowserPane control API is not available");
        }
        await control.refreshSessions({ preserveSelection: true, silent: true });
        await control.selectSession(selectedSessionId);
        await control.connectSelected({ clientRole: "recorder" });
      },
      sessionId,
    );
    await page.waitForFunction(
      () => window.__bpaneControl?.getState?.()?.connected === true,
      { timeout: this.connectTimeoutMs },
    );
    await page.waitForSelector("#desktop-container canvas", { timeout: this.connectTimeoutMs });
    await page.evaluate(() => {
      const recording = window.__bpaneRecording;
      if (!recording) {
        throw new Error("BrowserPane recording API is not available");
      }
      recording.setAutoDownload(false);
      return recording.start();
    });
    this.startedAtMs = Date.now();
  }

  async stopAndDownload(outputPath: string): Promise<RecordingArtifact> {
    const page = this.requirePage();
    const stopResult = await page.evaluate(async () => {
      const recording = window.__bpaneRecording;
      if (!recording) {
        throw new Error("BrowserPane recording API is not available");
      }
      const blob = await recording.stop();
      return { size: blob?.size ?? 0, type: blob?.type ?? "" };
    });
    if (!stopResult.size) {
      throw new Error("recording finalized without any media bytes");
    }

    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    const [download] = await Promise.all([
      page.waitForEvent("download"),
      page.evaluate(() => {
        const recording = window.__bpaneRecording;
        if (!recording) {
          throw new Error("BrowserPane recording API is not available");
        }
        recording.downloadLast();
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
          if (window.__bpaneControl?.getState?.()?.connected) {
            await window.__bpaneControl.disconnect();
          }
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
      "--origin-to-force-quic-on=localhost:4433",
      "--disable-background-timer-throttling",
      "--disable-renderer-backgrounding",
      "--disable-backgrounding-occluded-windows",
    ];
    if (this.certSpki) {
      args.push(`--ignore-certificate-errors-spki-list=${this.certSpki}`);
    }
    return args;
  }

  private buildRecorderPageUrl(): string {
    const url = new URL(this.pageUrl);
    url.searchParams.set("layout", "browser-only");
    url.searchParams.set("client_role", "recorder");
    return url.toString();
  }

  private async ensureLoggedIn(page: Page): Promise<void> {
    const state = await page.evaluate(() => ({
      configured: window.__bpaneAuth?.isConfigured?.() ?? false,
      authenticated: window.__bpaneAuth?.isAuthenticated?.() ?? false,
      exampleUser: window.__bpaneAuth?.getExampleUser?.() ?? null,
    }));
    if (!state.configured || state.authenticated) {
      return;
    }
    if (!state.exampleUser?.username || !state.exampleUser?.password) {
      throw new Error("OIDC auth is enabled, but no example user is configured");
    }

    await page.click("#btn-login");
    const username = page.locator('input[name="username"], #username').first();
    const password = page.locator('input[name="password"], #password').first();
    await username.waitFor({ state: "visible", timeout: this.connectTimeoutMs });
    await password.waitFor({ state: "visible", timeout: this.connectTimeoutMs });
    await username.fill(state.exampleUser.username);
    await password.fill(state.exampleUser.password);
    await page.locator('input[type="submit"], #kc-login').click();
    await page.waitForFunction(() => window.__bpaneAuth?.isAuthenticated?.() === true, {
      timeout: this.connectTimeoutMs,
    });
  }

  private requirePage(): Page {
    if (!this.page) {
      throw new Error("recorder page is not active");
    }
    return this.page;
  }
}
