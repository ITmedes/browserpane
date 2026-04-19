import { describe, expect, it, vi } from 'vitest';
import { AtomicMacShortcutDispatcher } from '../input/atomic-mac-shortcut-dispatcher.js';

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

function createDispatcher() {
  const emitted: EmittedKeyEvent[] = [];
  const syncClipboardBeforePaste = vi.fn(() => Promise.resolve());
  const dispatcher = new AtomicMacShortcutDispatcher({
    getPreferredCtrlCode: () => 'ControlRight',
    emitKeyEvent: (event) => {
      emitted.push(event);
    },
    syncClipboardBeforePaste,
  });

  return {
    dispatcher,
    emitted,
    syncClipboardBeforePaste,
  };
}

describe('AtomicMacShortcutDispatcher', () => {
  it('dispatches an atomic control shortcut with the preferred control key', () => {
    const { dispatcher, emitted } = createDispatcher();

    dispatcher.dispatchShortcut({
      code: 'KeyX',
      key: 'x',
    });

    expect(emitted).toEqual([
      {
        code: 'ControlRight',
        key: '',
        down: true,
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyX',
        key: 'x',
        down: true,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'KeyX',
        key: 'x',
        down: false,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        altgr: false,
      },
      {
        code: 'ControlRight',
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

  it('dispatches non-paste shortcuts immediately without clipboard sync', () => {
    const { dispatcher, emitted, syncClipboardBeforePaste } = createDispatcher();

    dispatcher.dispatchShortcutWithClipboardSync({
      code: 'KeyC',
      key: 'c',
      clipboardEnabled: true,
    });

    expect(syncClipboardBeforePaste).not.toHaveBeenCalled();
    expect(emitted).toHaveLength(4);
  });

  it('waits for clipboard sync before dispatching paste shortcuts', async () => {
    let resolveClipboard = () => {};
    const emitted: EmittedKeyEvent[] = [];
    const dispatcher = new AtomicMacShortcutDispatcher({
      getPreferredCtrlCode: () => 'ControlLeft',
      emitKeyEvent: (event) => {
        emitted.push(event);
      },
      syncClipboardBeforePaste: () => new Promise<void>((resolve) => {
        resolveClipboard = resolve;
      }),
    });

    dispatcher.dispatchShortcutWithClipboardSync({
      code: 'KeyV',
      key: 'v',
      clipboardEnabled: true,
    });

    expect(emitted).toEqual([]);

    resolveClipboard();
    await Promise.resolve();
    await Promise.resolve();

    expect(emitted.map(({ code, down, ctrl }) => ({
      code,
      down,
      ctrl,
    }))).toEqual([
      { code: 'ControlLeft', down: true, ctrl: false },
      { code: 'KeyV', down: true, ctrl: true },
      { code: 'KeyV', down: false, ctrl: true },
      { code: 'ControlLeft', down: false, ctrl: false },
    ]);
  });

  it('still dispatches paste shortcuts when clipboard sync fails', async () => {
    const emitted: EmittedKeyEvent[] = [];
    const dispatcher = new AtomicMacShortcutDispatcher({
      getPreferredCtrlCode: () => 'ControlLeft',
      emitKeyEvent: (event) => {
        emitted.push(event);
      },
      syncClipboardBeforePaste: () => Promise.reject(new Error('clipboard unavailable')),
    });

    dispatcher.dispatchShortcutWithClipboardSync({
      code: 'KeyV',
      key: 'v',
      clipboardEnabled: true,
    });

    await Promise.resolve();
    await Promise.resolve();

    expect(emitted.map(({ code, down, ctrl }) => ({
      code,
      down,
      ctrl,
    }))).toEqual([
      { code: 'ControlLeft', down: true, ctrl: false },
      { code: 'KeyV', down: true, ctrl: true },
      { code: 'KeyV', down: false, ctrl: true },
      { code: 'ControlLeft', down: false, ctrl: false },
    ]);
  });
});
