import { describe, expect, it } from 'vitest';
import { DeferredCtrlPasteRuntime } from '../input/deferred-ctrl-paste-runtime.js';

function createRuntime() {
  const emitted: Array<{ code: string; down: boolean; ctrl: boolean }> = [];
  const runtime = new DeferredCtrlPasteRuntime({
    emitKeyEvent: (code, down, ctrl) => {
      emitted.push({ code, down, ctrl });
    },
  });

  return {
    runtime,
    emitted,
  };
}

describe('DeferredCtrlPasteRuntime', () => {
  it('defers Ctrl+V only while clipboard sync is enabled and a control key is held', () => {
    const { runtime } = createRuntime();

    expect(runtime.shouldDeferPaste({
      code: 'KeyV',
      ctrlKey: true,
      altKey: false,
      metaKey: false,
      shiftKey: false,
    }, true)).toBe(false);

    runtime.noteControlKeydown('ControlLeft');

    expect(runtime.shouldDeferPaste({
      code: 'KeyV',
      ctrlKey: true,
      altKey: false,
      metaKey: false,
      shiftKey: false,
    }, true)).toBe(true);
    expect(runtime.shouldDeferPaste({
      code: 'KeyV',
      ctrlKey: true,
      altKey: false,
      metaKey: false,
      shiftKey: false,
    }, false)).toBe(false);
    expect(runtime.shouldDeferPaste({
      code: 'KeyV',
      ctrlKey: true,
      altKey: false,
      metaKey: false,
      shiftKey: true,
    }, true)).toBe(false);
  });

  it('captures held control keys and delays their release until flush', () => {
    const { runtime, emitted } = createRuntime();

    runtime.noteControlKeydown('ControlLeft');

    expect(runtime.begin('KeyV')).toBe(true);
    expect(runtime.noteControlKeyup('ControlLeft')).toBe(true);

    runtime.flush();

    expect(emitted).toEqual([
      { code: 'KeyV', down: true, ctrl: true },
      { code: 'KeyV', down: false, ctrl: true },
      { code: 'ControlLeft', down: false, ctrl: false },
    ]);
  });

  it('does not start a second deferred paste while one is already pending', () => {
    const { runtime } = createRuntime();

    runtime.noteControlKeydown('ControlLeft');

    expect(runtime.begin('KeyV')).toBe(true);
    expect(runtime.begin('KeyV')).toBe(false);
  });

  it('clears active and pending state on reset', () => {
    const { runtime, emitted } = createRuntime();

    runtime.noteControlKeydown('ControlLeft');
    expect(runtime.begin('KeyV')).toBe(true);
    expect(runtime.noteControlKeyup('ControlLeft')).toBe(true);

    runtime.reset();
    runtime.flush();

    expect(emitted).toEqual([]);
    expect(runtime.shouldDeferPaste({
      code: 'KeyV',
      ctrlKey: true,
      altKey: false,
      metaKey: false,
      shiftKey: false,
    }, true)).toBe(false);
  });
});
