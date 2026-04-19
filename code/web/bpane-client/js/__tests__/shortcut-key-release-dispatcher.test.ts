import { describe, expect, it, vi } from 'vitest';
import { ShortcutKeyReleaseRuntime } from '../input/shortcut-key-release-runtime.js';
import { ShortcutKeyReleaseDispatcher } from '../input/shortcut-key-release-dispatcher.js';

function createDispatcher() {
  const runtime = new ShortcutKeyReleaseRuntime();
  const emitted: Array<{
    code: string;
    key: string;
    ctrl: boolean;
    alt: boolean;
    shift: boolean;
    meta: boolean;
    altgr: boolean;
  }> = [];
  const suppressedCodes: string[] = [];
  const dispatcher = new ShortcutKeyReleaseDispatcher({
    runtime,
    emitKeyRelease: (release) => {
      emitted.push(release);
    },
    suppressKeyup: (code) => {
      suppressedCodes.push(code);
    },
  });

  return {
    runtime,
    dispatcher,
    emitted,
    suppressedCodes,
  };
}

describe('ShortcutKeyReleaseDispatcher', () => {
  it('releases tracked shortcut keys and suppresses their later keyups', () => {
    const {
      runtime,
      dispatcher,
      emitted,
      suppressedCodes,
    } = createDispatcher();
    runtime.noteSentKeydown({
      code: 'KeyL',
      key: 'l',
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
    runtime.noteSentKeydown({
      code: 'KeyV',
      key: 'v',
      ctrl: false,
      alt: false,
      shift: false,
      meta: true,
      altgr: false,
    });

    dispatcher.releaseKeysForModifierKeyup({
      code: 'MetaLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: false,
    });

    expect(emitted).toEqual([
      {
        code: 'KeyL',
        key: 'l',
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyV',
        key: 'v',
        ctrl: false,
        alt: false,
        shift: false,
        meta: true,
        altgr: false,
      },
    ]);
    expect(suppressedCodes).toEqual(['KeyL', 'KeyV']);
  });

  it('does nothing when the runtime has no pending shortcut releases', () => {
    const { dispatcher, emitted, suppressedCodes } = createDispatcher();

    dispatcher.releaseKeysForModifierKeyup({
      code: 'MetaLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: false,
    });

    expect(emitted).toEqual([]);
    expect(suppressedCodes).toEqual([]);
  });

  it('ignores non-final modifier keyups that should not release tracked keys', () => {
    const { runtime, dispatcher, emitted, suppressedCodes } = createDispatcher();
    runtime.noteSentKeydown({
      code: 'KeyL',
      key: 'l',
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });

    dispatcher.releaseKeysForModifierKeyup({
      code: 'MetaLeft',
      ctrlKey: false,
      altKey: false,
      metaKey: true,
    });

    expect(emitted).toEqual([]);
    expect(suppressedCodes).toEqual([]);
  });
});
