import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fnvHash } from '../hash.js';
import { ClipboardSyncRuntime } from '../input/clipboard-sync-runtime.js';

function createPasteEvent(text: string): ClipboardEvent {
  const event = new Event('paste', { bubbles: true, cancelable: true }) as ClipboardEvent;
  Object.defineProperty(event, 'clipboardData', {
    configurable: true,
    value: {
      getData: (type: string) => (type === 'text/plain' ? text : ''),
    },
  });
  return event;
}

function createRuntime(options: {
  clipboardText?: string;
  rejectRead?: boolean;
} = {}) {
  const canvas = document.createElement('canvas');
  const keyboardTarget = document.createElement('textarea');
  document.body.append(canvas, keyboardTarget);

  const sendClipboardText = vi.fn();
  let lastClipboardHash = 0n;
  const readText = options.rejectRead
    ? vi.fn().mockRejectedValue(new Error('clipboard denied'))
    : vi.fn().mockResolvedValue(options.clipboardText ?? '');

  const runtime = new ClipboardSyncRuntime({
    canvas,
    sendClipboardText,
    getLastClipboardHash: () => lastClipboardHash,
    setLastClipboardHash: (hash) => {
      lastClipboardHash = hash;
    },
    navigatorLike: {
      clipboard: {
        readText,
      },
    },
    documentLike: document,
    scheduleTimeout: (callback, delayMs) => window.setTimeout(callback, delayMs),
  });

  return {
    canvas,
    keyboardTarget,
    runtime,
    readText,
    sendClipboardText,
    getLastClipboardHash: () => lastClipboardHash,
  };
}

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

beforeEach(() => {
  document.body.innerHTML = '';
  vi.useRealTimers();
});

describe('ClipboardSyncRuntime', () => {
  it('refreshes changed clipboard text without requiring a paste shortcut wait', async () => {
    const { runtime, readText, sendClipboardText, getLastClipboardHash } = createRuntime({
      clipboardText: 'background refresh',
    });

    await runtime.refreshClipboardText();

    expect(readText).toHaveBeenCalledTimes(1);
    expect(sendClipboardText).toHaveBeenCalledWith('background refresh');
    expect(getLastClipboardHash()).toBe(fnvHash('background refresh'));
  });

  it('reads and sends changed clipboard text before paste shortcuts', async () => {
    const { runtime, readText, sendClipboardText, getLastClipboardHash } = createRuntime({
      clipboardText: 'fresh paste',
    });

    await runtime.syncClipboardBeforePaste();

    expect(readText).toHaveBeenCalledTimes(1);
    expect(sendClipboardText).toHaveBeenCalledWith('fresh paste');
    expect(getLastClipboardHash()).toBe(fnvHash('fresh paste'));
  });

  it('ignores missing, duplicate, and rejected clipboard reads', async () => {
    const duplicate = createRuntime({
      clipboardText: 'same text',
    });
    await duplicate.runtime.syncClipboardBeforePaste();
    await duplicate.runtime.syncClipboardBeforePaste();

    expect(duplicate.sendClipboardText).toHaveBeenCalledTimes(1);

    const missing = createRuntime({
      clipboardText: '',
    });
    await missing.runtime.syncClipboardBeforePaste();

    expect(missing.sendClipboardText).not.toHaveBeenCalled();

    const rejected = createRuntime({
      rejectRead: true,
    });
    await rejected.runtime.syncClipboardBeforePaste();

    expect(rejected.sendClipboardText).not.toHaveBeenCalled();
  });

  it('forwards pasted plain text from the keyboard target', () => {
    const { keyboardTarget, runtime, sendClipboardText, getLastClipboardHash } = createRuntime();
    const abortController = new AbortController();

    runtime.bind({
      keyboardTarget,
      signal: abortController.signal,
    });

    const event = createPasteEvent('pasted text');
    keyboardTarget.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(sendClipboardText).toHaveBeenCalledWith('pasted text');
    expect(getLastClipboardHash()).toBe(fnvHash('pasted text'));
  });

  it('reads clipboard text after copy and cut events', async () => {
    vi.useFakeTimers();
    const { keyboardTarget, runtime, readText, sendClipboardText } = createRuntime({
      clipboardText: 'copied text',
    });
    const abortController = new AbortController();

    runtime.bind({
      keyboardTarget,
      signal: abortController.signal,
    });

    document.dispatchEvent(new Event('copy'));
    await vi.advanceTimersByTimeAsync(50);
    await flushPromises();

    expect(readText).toHaveBeenCalledTimes(1);
    expect(sendClipboardText).toHaveBeenCalledWith('copied text');

    document.dispatchEvent(new Event('cut'));
    await vi.advanceTimersByTimeAsync(50);
    await flushPromises();

    expect(readText).toHaveBeenCalledTimes(2);
  });

  it('invokes a bare scheduleTimeout function with the global receiver', () => {
    const canvas = document.createElement('canvas');
    let lastClipboardHash = 0n;
    const scheduled = new Map<number, () => void>();
    let nextTimerId = 1;
    let seenThis = false;
    const scheduleTimeout = function(this: unknown, handler: () => void, delayMs: number): number {
      seenThis = this === globalThis;
      if (this !== globalThis) {
        throw new TypeError('Illegal invocation');
      }
      expect(delayMs).toBe(50);
      const timerId = nextTimerId++;
      scheduled.set(timerId, handler);
      return timerId;
    };

    const runtime = new ClipboardSyncRuntime({
      canvas,
      sendClipboardText: vi.fn(),
      getLastClipboardHash: () => lastClipboardHash,
      setLastClipboardHash: (hash) => {
        lastClipboardHash = hash;
      },
      navigatorLike: {
        clipboard: {
          readText: vi.fn().mockResolvedValue('copied text'),
        },
      },
      documentLike: document,
      scheduleTimeout: scheduleTimeout as (callback: () => void, delayMs: number) => number,
    });
    const abortController = new AbortController();
    const keyboardTarget = document.createElement('textarea');

    runtime.bind({
      keyboardTarget,
      signal: abortController.signal,
    });

    document.dispatchEvent(new Event('copy'));

    expect(seenThis).toBe(true);
    expect(scheduled.size).toBe(1);
  });
});
