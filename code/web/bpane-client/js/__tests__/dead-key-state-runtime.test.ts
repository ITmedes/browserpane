import { describe, expect, it, vi } from 'vitest';
import { DeadKeyStateRuntime } from '../input/dead-key-state-runtime.js';

function createRuntime() {
  const resetPendingComposition = vi.fn();
  const clearKeyboardSink = vi.fn();
  const beginPendingComposition = vi.fn();
  const emitted: Array<{ code: string; key: string; down: boolean }> = [];
  const runtime = new DeadKeyStateRuntime({
    resetPendingComposition,
    clearKeyboardSink,
    beginPendingComposition,
    emitSyntheticKeyEvent: (code, key, down) => {
      emitted.push({ code, key, down });
    },
  });

  return {
    runtime,
    resetPendingComposition,
    clearKeyboardSink,
    beginPendingComposition,
    emitted,
  };
}

describe('DeadKeyStateRuntime', () => {
  it('starts supported dead accents by clearing composition state and keyboard sink', () => {
    const {
      runtime,
      resetPendingComposition,
      clearKeyboardSink,
    } = createRuntime();

    runtime.startSupportedDeadAccent('Equal');

    expect(resetPendingComposition).toHaveBeenCalledTimes(1);
    expect(clearKeyboardSink).toHaveBeenCalledTimes(1);
    expect(runtime.shouldIgnoreComposingKeydown(true)).toBe(true);
  });

  it('converts the next keydown after a native dead key into a pending composition start', () => {
    const { runtime, beginPendingComposition } = createRuntime();

    runtime.noteNativeDeadKey('KeyE');

    expect(runtime.shouldIgnoreComposingKeydown(true)).toBe(false);
    expect(runtime.beginPendingCompositionIfNeeded({
      code: 'KeyE',
      shift: false,
      fallbackKey: 'e',
    })).toBe(true);
    expect(beginPendingComposition).toHaveBeenCalledWith({
      code: 'KeyE',
      shift: false,
      fallbackKey: 'e',
    });
    expect(runtime.beginPendingCompositionIfNeeded({
      code: 'KeyE',
      shift: false,
      fallbackKey: 'e',
    })).toBe(false);
  });

  it('emits spacing accent fallback keys and tracks the dead key for suppression', () => {
    const { runtime, emitted } = createRuntime();

    runtime.applySyntheticAccentFallback({
      deadCode: 'Equal',
      spacingAccent: '´',
      deadKeyCode: 'Equal',
    });

    expect(emitted).toEqual([
      { code: 'Equal', key: '´', down: true },
      { code: 'Equal', key: '´', down: false },
    ]);
    expect(runtime.consumeTrackedDeadKeyKeyup('Equal')).toBe(true);
    expect(runtime.consumeTrackedDeadKeyKeyup('Equal')).toBe(false);
  });

  it('clears tracked dead-key state explicitly and on reset', () => {
    const { runtime } = createRuntime();

    runtime.noteNativeDeadKey('Backquote');
    runtime.clearTrackedDeadKey();
    expect(runtime.consumeTrackedDeadKeyKeyup('Backquote')).toBe(false);

    runtime.noteNativeDeadKey('Backquote');
    runtime.reset();
    expect(runtime.beginPendingCompositionIfNeeded({
      code: 'KeyA',
      shift: false,
      fallbackKey: 'a',
    })).toBe(false);
    expect(runtime.consumeTrackedDeadKeyKeyup('Backquote')).toBe(false);
  });
});
