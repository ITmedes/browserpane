import { describe, expect, it } from 'vitest';
import { ShortcutKeyReleaseRuntime } from '../input/shortcut-key-release-runtime.js';

function createRuntime() {
  return new ShortcutKeyReleaseRuntime();
}

describe('ShortcutKeyReleaseRuntime', () => {
  it('tracks sent non-modifier shortcut keydowns', () => {
    const runtime = createRuntime();

    runtime.noteSentKeydown({
      code: 'KeyL',
      key: 'l',
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });

    expect(runtime.releaseKeysForModifierKeyup({
      code: 'ControlLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: false,
    })).toEqual([
      {
        code: 'KeyL',
        key: 'l',
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
    ]);
  });

  it('does not track plain keys or altgr text composition', () => {
    const runtime = createRuntime();

    runtime.noteSentKeydown({
      code: 'KeyA',
      key: 'a',
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
    runtime.noteSentKeydown({
      code: 'KeyQ',
      key: '@',
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
      altgr: true,
    });

    expect(runtime.releaseKeysForModifierKeyup({
      code: 'ControlLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: false,
    })).toEqual([]);
  });

  it('clears tracked keys on real keyup and when another shortcut modifier is still held', () => {
    const runtime = createRuntime();

    runtime.noteSentKeydown({
      code: 'KeyL',
      key: 'l',
      ctrl: true,
      alt: false,
      shift: true,
      meta: false,
      altgr: false,
    });

    expect(runtime.releaseKeysForModifierKeyup({
      code: 'ShiftLeft',
      ctrlKey: true,
      altKey: false,
      metaKey: false,
    })).toEqual([]);

    runtime.noteObservedKeyup('KeyL');

    expect(runtime.releaseKeysForModifierKeyup({
      code: 'ControlLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: false,
    })).toEqual([]);
  });

  it('resets all tracked shortcut keys', () => {
    const runtime = createRuntime();

    runtime.noteSentKeydown({
      code: 'KeyR',
      key: 'r',
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
    runtime.reset();

    expect(runtime.releaseKeysForModifierKeyup({
      code: 'ControlLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: false,
    })).toEqual([]);
  });
});
