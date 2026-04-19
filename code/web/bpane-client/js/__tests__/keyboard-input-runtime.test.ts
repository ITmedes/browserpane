import { beforeEach, describe, expect, it, vi } from 'vitest';

import { DeferredCtrlPasteRuntime } from '../input/deferred-ctrl-paste-runtime.js';
import { ShortcutKeyReleaseDispatcher } from '../input/shortcut-key-release-dispatcher.js';
import { ShortcutKeyReleaseRuntime } from '../input/shortcut-key-release-runtime.js';
import { KeyboardInputRuntime } from '../input/keyboard-input-runtime.js';

interface EmittedKeyEvent {
  code: string;
  key: string;
  down: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  altgr: boolean;
}

function dispatchKey(
  target: HTMLElement,
  type: 'keydown' | 'keyup',
  init: KeyboardEventInit & { code: string; key: string },
): KeyboardEvent {
  const event = new KeyboardEvent(type, {
    bubbles: true,
    cancelable: true,
    ...init,
  });
  target.dispatchEvent(event);
  return event;
}

function createRuntime(options: {
  clipboardEnabled?: boolean;
  syncClipboardBeforePaste?: () => Promise<void>;
  shouldSendAtomicMacCtrlShortcut?: (event: KeyboardEvent) => boolean;
} = {}) {
  const emittedKeyEvents: EmittedKeyEvent[] = [];
  const target = document.createElement('textarea');
  const signal = new AbortController().signal;
  const suppressions = new Set<string>();
  const atomicShortcuts: Array<{ code: string; key: string; clipboardEnabled: boolean }> = [];
  const releaseRuntime = new ShortcutKeyReleaseRuntime();
  const deferredCtrlPaste = new DeferredCtrlPasteRuntime({
    emitKeyEvent: (code, down, ctrl) => {
      emittedKeyEvents.push({
        code,
        key: '',
        down,
        ctrl,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      });
    },
  });

  const runtime = new KeyboardInputRuntime({
    clipboardEnabled: options.clipboardEnabled ?? true,
    isMac: true,
    macMetaAsCtrl: true,
    keyboardSink: {
      clear: vi.fn(),
    },
    suppressedKeyups: {
      clear: (code: string) => suppressions.delete(code),
      suppress: (code: string) => {
        suppressions.add(code);
      },
    },
    macModifiers: {
      isMacMetaKey: vi.fn().mockReturnValue(false),
      noteMetaKeydown: vi.fn(),
      isMacOptionKey: vi.fn().mockReturnValue(false),
      noteOptionKeydown: vi.fn(),
      releaseMacCtrlsForRemap: vi.fn().mockReturnValue([]),
      isMacOptionComposition: vi.fn().mockReturnValue(false),
      shouldMaterializeMacCtrl: vi.fn().mockReturnValue(false),
      materializeMacCtrl: vi.fn(),
      shouldMaterializeMacOption: vi.fn().mockReturnValue(false),
      materializeMacOption: vi.fn(),
      handleMetaKeyup: vi.fn().mockReturnValue(false),
      handleOptionKeyup: vi.fn().mockReturnValue(false),
    },
    deferredCtrlPaste,
    macNavigationRemap: {
      hasActiveRemap: vi.fn().mockReturnValue(false),
      handleKeyup: vi.fn().mockReturnValue(false),
    },
    resolveSupportedDeadAccent: vi.fn().mockReturnValue(null),
    deadKeyState: {
      startSupportedDeadAccent: vi.fn(),
      applySyntheticAccentFallback: vi.fn(),
      shouldIgnoreComposingKeydown: vi.fn().mockReturnValue(false),
      noteNativeDeadKey: vi.fn(),
      beginPendingCompositionIfNeeded: vi.fn().mockReturnValue(false),
      clearTrackedDeadKey: vi.fn(),
      consumeTrackedDeadKeyKeyup: vi.fn().mockReturnValue(false),
    },
    syntheticDeadAccent: {
      begin: vi.fn(),
      handleKeydown: vi.fn().mockReturnValue({ handled: false }),
      handleKeyup: vi.fn().mockReturnValue({ handled: false, clearDeadKeyCode: false }),
    },
    shortcutPolicy: {
      shouldSuppressLockedWindowShortcut: vi.fn().mockReturnValue(false),
      shouldPassThroughMacMetaShortcut: vi.fn().mockReturnValue(false),
      shouldSendAtomicMacCtrlShortcut: vi.fn(
        options.shouldSendAtomicMacCtrlShortcut ?? (() => false),
      ),
    },
    atomicMacShortcutDispatcher: {
      dispatchShortcutWithClipboardSync: vi.fn((shortcut) => {
        atomicShortcuts.push(shortcut);
      }),
    },
    clipboardSync: {
      syncClipboardBeforePaste: options.syncClipboardBeforePaste ?? vi.fn().mockResolvedValue(undefined),
      refreshClipboardText: vi.fn().mockResolvedValue(undefined),
    },
    macNavigationShortcutDispatcher: {
      dispatchShortcut: vi.fn(),
    },
    pendingComposition: {
      hasPendingCode: vi.fn().mockReturnValue(false),
      handleKeyup: vi.fn().mockReturnValue(false),
    },
    keyEventStateResolver: {
      resolve: vi.fn((event: KeyboardEvent) => ({
        ctrl: event.ctrlKey || event.metaKey,
        alt: event.altKey,
        shift: event.shiftKey,
        meta: false,
        altgr: false,
      })),
    },
    shortcutKeyRelease: releaseRuntime,
    shortcutKeyReleaseDispatcher: new ShortcutKeyReleaseDispatcher({
      runtime: releaseRuntime,
      emitKeyRelease: (release) => {
        emittedKeyEvents.push({
          code: release.code,
          key: release.key,
          down: false,
          ctrl: release.ctrl,
          alt: release.alt,
          shift: release.shift,
          meta: release.meta,
          altgr: release.altgr,
        });
      },
      suppressKeyup: (code) => {
        suppressions.add(code);
      },
    }),
    emitKeyEvent: (event) => {
      emittedKeyEvents.push(event);
      return true;
    },
  });

  runtime.bind({
    keyboardTarget: target,
    signal,
  });

  return {
    emittedKeyEvents,
    atomicShortcuts,
    runtime,
    target,
  };
}

beforeEach(() => {
  document.body.innerHTML = '';
});

describe('KeyboardInputRuntime', () => {
  it('dispatches atomic mac shortcuts only on the first non-repeat keydown', () => {
    const { atomicShortcuts, emittedKeyEvents, target } = createRuntime({
      shouldSendAtomicMacCtrlShortcut: (event) => event.code === 'KeyV' && event.metaKey,
    });

    const first = dispatchKey(target, 'keydown', {
      code: 'KeyV',
      key: 'v',
      metaKey: true,
    });
    const repeated = dispatchKey(target, 'keydown', {
      code: 'KeyV',
      key: 'v',
      metaKey: true,
      repeat: true,
    });

    expect(first.defaultPrevented).toBe(true);
    expect(repeated.defaultPrevented).toBe(true);
    expect(atomicShortcuts).toEqual([
      { code: 'KeyV', key: 'v', clipboardEnabled: true },
    ]);
    expect(emittedKeyEvents).toEqual([]);
  });

  it('waits for deferred Ctrl+V clipboard sync before emitting paste and releasing control', async () => {
    let resolveClipboard = () => {};
    const { emittedKeyEvents, target } = createRuntime({
      syncClipboardBeforePaste: () => new Promise<void>((resolve) => {
        resolveClipboard = resolve;
      }),
    });

    const ctrlDown = dispatchKey(target, 'keydown', {
      code: 'ControlLeft',
      key: 'Control',
      ctrlKey: true,
    });
    const pasteDown = dispatchKey(target, 'keydown', {
      code: 'KeyV',
      key: 'v',
      ctrlKey: true,
    });
    const ctrlUp = dispatchKey(target, 'keyup', {
      code: 'ControlLeft',
      key: 'Control',
    });
    const pasteUp = dispatchKey(target, 'keyup', {
      code: 'KeyV',
      key: 'v',
      ctrlKey: true,
    });

    expect(ctrlDown.defaultPrevented).toBe(true);
    expect(pasteDown.defaultPrevented).toBe(true);
    expect(ctrlUp.defaultPrevented).toBe(true);
    expect(pasteUp.defaultPrevented).toBe(true);
    expect(emittedKeyEvents).toEqual([
      {
        code: 'ControlLeft',
        key: 'Control',
        down: true,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
    ]);

    resolveClipboard();
    await Promise.resolve();
    await Promise.resolve();

    expect(emittedKeyEvents).toEqual([
      {
        code: 'ControlLeft',
        key: 'Control',
        down: true,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyV',
        key: '',
        down: true,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyV',
        key: '',
        down: false,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'ControlLeft',
        key: '',
        down: false,
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
    ]);
  });

  it('synthesizes tracked shortcut key releases when the modifier keyup arrives first', () => {
    const { emittedKeyEvents, target } = createRuntime();

    dispatchKey(target, 'keydown', {
      code: 'ControlLeft',
      key: 'Control',
      ctrlKey: true,
    });
    dispatchKey(target, 'keydown', {
      code: 'KeyL',
      key: 'l',
      ctrlKey: true,
    });
    const ctrlUp = dispatchKey(target, 'keyup', {
      code: 'ControlLeft',
      key: 'Control',
    });

    expect(ctrlUp.defaultPrevented).toBe(true);
    expect(emittedKeyEvents).toEqual([
      {
        code: 'ControlLeft',
        key: 'Control',
        down: true,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyL',
        key: 'l',
        down: true,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyL',
        key: 'l',
        down: false,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'ControlLeft',
        key: 'Control',
        down: false,
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
    ]);
  });
});
