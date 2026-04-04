/**
 * Polls the BrowserPane gateway HTTP API to track connected browser (supervisor) clients.
 * The MCP proxy uses this to decide whether to slow down Playwright execution.
 */
export class SupervisorMonitor {
  private browserClientCount = 0;
  private intervalId: ReturnType<typeof setInterval> | null = null;

  constructor(
    private gatewayApiUrl: string,
    private pollIntervalMs: number = 2000,
  ) {}

  start(): void {
    this.poll();
    this.intervalId = setInterval(() => this.poll(), this.pollIntervalMs);
  }

  stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }

  getBrowserClientCount(): number {
    return this.browserClientCount;
  }

  private async poll(): Promise<void> {
    try {
      const resp = await fetch(`${this.gatewayApiUrl}/api/session/status`);
      if (resp.ok) {
        const data = (await resp.json()) as {
          browser_clients: number;
          mcp_owner: boolean;
          resolution: [number, number];
        };
        this.browserClientCount = data.browser_clients ?? 0;
      }
    } catch {
      // Gateway unavailable — assume no supervisors
    }
  }
}
