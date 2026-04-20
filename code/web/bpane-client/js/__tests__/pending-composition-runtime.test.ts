import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PendingCompositionRuntime } from '../input/pending-composition-runtime.js';

function createRuntime() {
  const emitted: Array<{ code: string; key: string; shift: boolean }> = [];
  const suppressKeyup = vi.fn();
  const clearKeyboardSink = vi.fn();
  const runtime = new PendingCompositionRuntime({
    fallbackDelayMs: 16,
    setTimeoutFn: window.setTimeout,
    clearTimeoutFn: window.clearTimeout,
    emitCharacter: (input) => {
      emitted.push(input);
    },
    suppressKeyup,
    clearKeyboardSink,
  });

  return {
    runtime,
    emitted,
    suppressKeyup,
    clearKeyboardSink,
  };
}

beforeEach(() => {
  vi.useRealTimers();
});

describe('PendingCompositionRuntime', () => {
  it('emits a single-character composition and suppresses the matching keyup', () => {
    const { runtime, emitted, suppressKeyup, clearKeyboardSink } = createRuntime();

    runtime.begin({
      code: 'KeyE',
      shift: false,
      fallbackKey: 'e',
    });
    runtime.commit('é');

    expect(emitted).toEqual([
      { code: 'KeyE', key: 'é', shift: false },
    ]);
    expect(suppressKeyup).toHaveBeenCalledWith('KeyE');
    expect(clearKeyboardSink).toHaveBeenCalledTimes(1);
    expect(runtime.hasPendingCode('KeyE')).toBe(false);
  });

  it('ignores empty and multi-character composition commits', () => {
    const { runtime, emitted, suppressKeyup, clearKeyboardSink } = createRuntime();

    runtime.begin({
      code: 'KeyE',
      shift: false,
      fallbackKey: 'e',
    });
    runtime.commit('');
    runtime.commit('ee');

    expect(emitted).toEqual([]);
    expect(suppressKeyup).not.toHaveBeenCalled();
    expect(clearKeyboardSink).not.toHaveBeenCalled();
    expect(runtime.hasPendingCode('KeyE')).toBe(true);
  });

  it('schedules a fallback emission when the matching key is released before text arrives', async () => {
    vi.useFakeTimers();
    const { runtime, emitted, suppressKeyup, clearKeyboardSink } = createRuntime();

    runtime.begin({
      code: 'KeyO',
      shift: true,
      fallbackKey: 'O',
    });

    expect(runtime.handleKeyup('KeyO')).toBe(true);

    await vi.advanceTimersByTimeAsync(16);

    expect(emitted).toEqual([
      { code: 'KeyO', key: 'O', shift: true },
    ]);
    expect(suppressKeyup).not.toHaveBeenCalled();
    expect(clearKeyboardSink).toHaveBeenCalledTimes(1);
    expect(runtime.hasPendingCode('KeyO')).toBe(false);
  });

  it('drops non-printable fallback keys after the timer fires', async () => {
    vi.useFakeTimers();
    const { runtime, emitted, clearKeyboardSink } = createRuntime();

    runtime.begin({
      code: 'ArrowLeft',
      shift: false,
      fallbackKey: 'ArrowLeft',
    });

    expect(runtime.handleKeyup('ArrowLeft')).toBe(true);

    await vi.advanceTimersByTimeAsync(16);

    expect(emitted).toEqual([]);
    expect(clearKeyboardSink).toHaveBeenCalledTimes(1);
    expect(runtime.hasPendingCode('ArrowLeft')).toBe(false);
  });

  it('reset clears pending fallback timers', async () => {
    vi.useFakeTimers();
    const { runtime, emitted, clearKeyboardSink } = createRuntime();

    runtime.begin({
      code: 'KeyI',
      shift: false,
      fallbackKey: 'i',
    });
    expect(runtime.handleKeyup('KeyI')).toBe(true);

    runtime.reset();
    await vi.advanceTimersByTimeAsync(16);

    expect(emitted).toEqual([]);
    expect(clearKeyboardSink).not.toHaveBeenCalled();
    expect(runtime.hasPendingCode('KeyI')).toBe(false);
  });
});
