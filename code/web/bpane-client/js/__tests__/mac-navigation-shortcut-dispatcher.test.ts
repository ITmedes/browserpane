import { describe, expect, it } from 'vitest';
import { MacNavigationRemapRuntime } from '../input/mac-navigation-remap-runtime.js';
import { MacNavigationShortcutDispatcher } from '../input/mac-navigation-shortcut-dispatcher.js';

function createDispatcher() {
  const remapRuntime = new MacNavigationRemapRuntime();
  const emitted: Array<{ code: 'Home' | 'End'; down: boolean; shift: boolean }> = [];
  const restored: string[][] = [];
  let releasedCtrlCodes = ['ControlLeft'];
  const dispatcher = new MacNavigationShortcutDispatcher({
    remapRuntime,
    releaseMacCtrlsForRemap: () => [...releasedCtrlCodes],
    restoreMacCtrls: (ctrlCodes) => {
      restored.push([...ctrlCodes]);
    },
    emitNavigationKey: (code, down, shift) => {
      emitted.push({ code, down, shift });
    },
  });

  return {
    dispatcher,
    remapRuntime,
    emitted,
    restored,
    setReleasedCtrlCodes: (codes: string[]) => {
      releasedCtrlCodes = codes;
    },
  };
}

describe('MacNavigationShortcutDispatcher', () => {
  it('dispatches ArrowLeft as an atomic Home key and restores released controls', () => {
    const { dispatcher, remapRuntime, emitted, restored } = createDispatcher();

    expect(dispatcher.dispatchShortcut('ArrowLeft', true)).toBe(true);

    expect(emitted).toEqual([
      { code: 'Home', down: true, shift: true },
      { code: 'Home', down: false, shift: true },
    ]);
    expect(restored).toEqual([
      ['ControlLeft'],
    ]);
    expect(remapRuntime.hasActiveRemap('ArrowLeft')).toBe(true);
  });

  it('dispatches ArrowRight as an atomic End key', () => {
    const {
      dispatcher,
      emitted,
      restored,
      setReleasedCtrlCodes,
    } = createDispatcher();
    setReleasedCtrlCodes(['ControlRight']);

    expect(dispatcher.dispatchShortcut('ArrowRight', false)).toBe(true);

    expect(emitted).toEqual([
      { code: 'End', down: true, shift: false },
      { code: 'End', down: false, shift: false },
    ]);
    expect(restored).toEqual([
      ['ControlRight'],
    ]);
  });

  it('restores released controls even when the remap is rejected', () => {
    const { dispatcher, remapRuntime, emitted, restored } = createDispatcher();

    expect(dispatcher.dispatchShortcut('ArrowLeft', false)).toBe(true);
    expect(dispatcher.dispatchShortcut('ArrowLeft', false)).toBe(false);
    expect(dispatcher.dispatchShortcut('KeyA', false)).toBe(false);

    expect(emitted).toEqual([
      { code: 'Home', down: true, shift: false },
      { code: 'Home', down: false, shift: false },
    ]);
    expect(restored).toEqual([
      ['ControlLeft'],
      ['ControlLeft'],
      ['ControlLeft'],
    ]);
    expect(remapRuntime.hasActiveRemap('ArrowLeft')).toBe(true);
  });
});
