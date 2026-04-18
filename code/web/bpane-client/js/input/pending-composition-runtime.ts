export interface PendingCompositionInput {
  code: string;
  shift: boolean;
  fallbackKey: string;
}

export interface PendingCompositionRuntimeInput {
  fallbackDelayMs: number;
  setTimeoutFn: typeof window.setTimeout;
  clearTimeoutFn: typeof window.clearTimeout;
  emitCharacter: (input: { code: string; key: string; shift: boolean }) => void;
  suppressKeyup: (code: string) => void;
  clearKeyboardSink: () => void;
}

export class PendingCompositionRuntime {
  private readonly fallbackDelayMs: number;
  private readonly setTimeoutFn: typeof window.setTimeout;
  private readonly clearTimeoutFn: typeof window.clearTimeout;
  private readonly emitCharacter: (input: { code: string; key: string; shift: boolean }) => void;
  private readonly suppressKeyup: (code: string) => void;
  private readonly clearKeyboardSink: () => void;
  private pending: PendingCompositionInput | null = null;
  private fallbackTimer: number | null = null;

  constructor(input: PendingCompositionRuntimeInput) {
    this.fallbackDelayMs = input.fallbackDelayMs;
    this.setTimeoutFn = input.setTimeoutFn;
    this.clearTimeoutFn = input.clearTimeoutFn;
    this.emitCharacter = input.emitCharacter;
    this.suppressKeyup = input.suppressKeyup;
    this.clearKeyboardSink = input.clearKeyboardSink;
  }

  begin(input: PendingCompositionInput): void {
    this.clearFallbackTimer();
    this.pending = input;
  }

  hasPendingCode(code: string): boolean {
    return this.pending?.code === code;
  }

  commit(text: string): void {
    if (!this.pending || text.length !== 1) {
      return;
    }

    const pending = this.pending;
    this.pending = null;
    this.clearFallbackTimer();
    this.emitCharacter({
      code: pending.code,
      key: text,
      shift: pending.shift,
    });
    this.suppressKeyup(pending.code);
    this.clearKeyboardSink();
  }

  handleKeyup(code: string): boolean {
    if (this.pending?.code !== code) {
      return false;
    }

    this.clearFallbackTimer();
    this.fallbackTimer = Reflect.apply(this.setTimeoutFn, globalThis, [() => {
      this.fallbackTimer = null;
      const pending = this.pending;
      this.pending = null;
      if (pending?.fallbackKey.length === 1) {
        this.emitCharacter({
          code: pending.code,
          key: pending.fallbackKey,
          shift: pending.shift,
        });
      }
      this.clearKeyboardSink();
    }, this.fallbackDelayMs]);
    return true;
  }

  reset(): void {
    this.pending = null;
    this.clearFallbackTimer();
  }

  private clearFallbackTimer(): void {
    if (this.fallbackTimer !== null) {
      Reflect.apply(this.clearTimeoutFn, globalThis, [this.fallbackTimer]);
      this.fallbackTimer = null;
    }
  }
}
