import { fnvHash } from '../hash.js';

const CLIPBOARD_SYNC_DELAY_MS = 50;

export type ClipboardNavigatorLike = {
  clipboard?: {
    readText?: () => Promise<string>;
  };
};

export type ClipboardSyncRuntimeInput = {
  canvas: HTMLElement;
  sendClipboardText: (text: string) => void;
  getLastClipboardHash: () => bigint;
  setLastClipboardHash: (hash: bigint) => void;
  navigatorLike?: ClipboardNavigatorLike;
  documentLike?: Document;
  scheduleTimeout?: (callback: () => void, delayMs: number) => number;
};

export type ClipboardSyncBindingInput = {
  keyboardTarget: HTMLElement | null;
  signal: AbortSignal;
};

export class ClipboardSyncRuntime {
  private readonly canvas: HTMLElement;
  private readonly sendClipboardTextFn: (text: string) => void;
  private readonly getLastClipboardHash: () => bigint;
  private readonly setLastClipboardHashFn: (hash: bigint) => void;
  private readonly navigatorLike?: ClipboardNavigatorLike;
  private readonly documentLike?: Document;
  private readonly scheduleTimeoutFn: (callback: () => void, delayMs: number) => number;
  private listenersBound = false;

  private readonly handlePaste = (event: Event): void => {
    const clipboardEvent = event as ClipboardEvent;
    clipboardEvent.preventDefault();
    const text = clipboardEvent.clipboardData?.getData('text/plain') ?? '';
    this.maybeSendClipboardText(text);
  };

  private readonly handleCopy = (): void => {
    this.scheduleClipboardRead();
  };

  private readonly handleCut = (): void => {
    this.scheduleClipboardRead();
  };

  constructor(input: ClipboardSyncRuntimeInput) {
    this.canvas = input.canvas;
    this.sendClipboardTextFn = input.sendClipboardText;
    this.getLastClipboardHash = input.getLastClipboardHash;
    this.setLastClipboardHashFn = input.setLastClipboardHash;
    this.navigatorLike = input.navigatorLike;
    this.documentLike = input.documentLike;
    this.scheduleTimeoutFn = input.scheduleTimeout ?? ((callback, delayMs) => window.setTimeout(callback, delayMs));
  }

  bind(input: ClipboardSyncBindingInput): void {
    if (this.listenersBound) {
      return;
    }
    this.listenersBound = true;

    const pasteTarget = input.keyboardTarget ?? this.canvas;
    pasteTarget.addEventListener('paste', this.handlePaste, { signal: input.signal });
    this.documentLike?.addEventListener('copy', this.handleCopy, { signal: input.signal });
    this.documentLike?.addEventListener('cut', this.handleCut, { signal: input.signal });
  }

  reset(): void {
    this.listenersBound = false;
  }

  syncClipboardBeforePaste(): Promise<void> {
    return this.readAndSendClipboardText();
  }

  private scheduleTimeout(callback: () => void, delayMs: number): number {
    return Reflect.apply(this.scheduleTimeoutFn, globalThis, [callback, delayMs]);
  }

  private scheduleClipboardRead(): void {
    this.scheduleTimeout(() => {
      void this.readAndSendClipboardText();
    }, CLIPBOARD_SYNC_DELAY_MS);
  }

  private async readAndSendClipboardText(): Promise<void> {
    try {
      const text = await this.navigatorLike?.clipboard?.readText?.();
      this.maybeSendClipboardText(text ?? '');
    } catch {
      // Ignore clipboard permission and browser support failures.
    }
  }

  private maybeSendClipboardText(text: string): void {
    if (!text) {
      return;
    }
    const hash = fnvHash(text);
    if (hash === this.getLastClipboardHash()) {
      return;
    }
    this.setLastClipboardHashFn(hash);
    this.sendClipboardTextFn(text);
  }
}
