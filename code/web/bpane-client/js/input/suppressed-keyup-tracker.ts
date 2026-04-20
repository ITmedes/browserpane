export interface SuppressedKeyupTrackerInput {
  timeoutMs: number;
  setTimeoutFn: typeof window.setTimeout;
  clearTimeoutFn: typeof window.clearTimeout;
}

export class SuppressedKeyupTracker {
  private readonly timeoutMs: number;
  private readonly setTimeoutFn: typeof window.setTimeout;
  private readonly clearTimeoutFn: typeof window.clearTimeout;
  private readonly suppressedCodes = new Set<string>();
  private readonly timers = new Map<string, number>();

  constructor(input: SuppressedKeyupTrackerInput) {
    this.timeoutMs = input.timeoutMs;
    this.setTimeoutFn = input.setTimeoutFn;
    this.clearTimeoutFn = input.clearTimeoutFn;
  }

  private setTimeout(callback: () => void): number {
    return Reflect.apply(this.setTimeoutFn, globalThis, [callback, this.timeoutMs]);
  }

  private clearTimeout(timer: number): void {
    Reflect.apply(this.clearTimeoutFn, globalThis, [timer]);
  }

  suppress(code: string): void {
    this.suppressedCodes.add(code);
    const existingTimer = this.timers.get(code);
    if (existingTimer !== undefined) {
      this.clearTimeout(existingTimer);
    }
    const timer = this.setTimeout(() => {
      this.suppressedCodes.delete(code);
      this.timers.delete(code);
    });
    this.timers.set(code, timer);
  }

  clear(code: string): boolean {
    if (!this.suppressedCodes.has(code)) {
      return false;
    }
    this.suppressedCodes.delete(code);
    const timer = this.timers.get(code);
    if (timer !== undefined) {
      this.clearTimeout(timer);
      this.timers.delete(code);
    }
    return true;
  }

  reset(): void {
    this.suppressedCodes.clear();
    for (const timer of this.timers.values()) {
      this.clearTimeout(timer);
    }
    this.timers.clear();
  }
}
