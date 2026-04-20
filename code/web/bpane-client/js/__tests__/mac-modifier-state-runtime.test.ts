import { describe, expect, it } from 'vitest';
import { MacModifierStateRuntime } from '../input/mac-modifier-state-runtime.js';

function createRuntime(options: {
  isMac?: boolean;
  macMetaAsCtrl?: boolean;
} = {}) {
  const emitted: Array<{ code: string; down: boolean }> = [];
  const runtime = new MacModifierStateRuntime({
    isMac: options.isMac ?? true,
    macMetaAsCtrl: options.macMetaAsCtrl ?? true,
    emitModifierKey: (code, down) => {
      emitted.push({ code, down });
    },
  });

  return {
    runtime,
    emitted,
  };
}

describe('MacModifierStateRuntime', () => {
  it('recognizes mac meta and option keys only when enabled on macOS', () => {
    const enabled = createRuntime();
    expect(enabled.runtime.isMacMetaKey('MetaLeft')).toBe(true);
    expect(enabled.runtime.isMacMetaKey('MetaRight')).toBe(true);
    expect(enabled.runtime.isMacOptionKey('AltLeft')).toBe(true);
    expect(enabled.runtime.isMacOptionKey('AltRight')).toBe(true);

    const disabled = createRuntime({ macMetaAsCtrl: false });
    expect(disabled.runtime.isMacMetaKey('MetaLeft')).toBe(false);
    expect(disabled.runtime.isMacOptionKey('AltLeft')).toBe(true);

    const nonMac = createRuntime({ isMac: false, macMetaAsCtrl: false });
    expect(nonMac.runtime.isMacMetaKey('MetaLeft')).toBe(false);
    expect(nonMac.runtime.isMacOptionKey('AltLeft')).toBe(false);
  });

  it('materializes command as control and releases it on meta keyup', () => {
    const { runtime, emitted } = createRuntime();

    runtime.noteMetaKeydown('MetaLeft');
    expect(runtime.shouldMaterializeMacCtrl({ metaKey: true, code: 'KeyC' })).toBe(true);

    runtime.materializeMacCtrl();
    expect(runtime.preferredMacCtrlCode()).toBe('ControlLeft');
    expect(emitted).toEqual([
      { code: 'ControlLeft', down: true },
    ]);

    expect(runtime.handleMetaKeyup('MetaLeft')).toBe(true);
    expect(emitted).toEqual([
      { code: 'ControlLeft', down: true },
      { code: 'ControlLeft', down: false },
    ]);
  });

  it('releases and restores the matching materialized control for remaps', () => {
    const { runtime, emitted } = createRuntime();

    runtime.noteMetaKeydown('MetaRight');
    runtime.materializeMacCtrl();

    const released = runtime.releaseMacCtrlsForRemap();
    runtime.restoreMacCtrls(released);

    expect(released).toEqual(['ControlRight']);
    expect(emitted).toEqual([
      { code: 'ControlRight', down: true },
      { code: 'ControlRight', down: false },
      { code: 'ControlRight', down: true },
    ]);
  });

  it('treats option-printable input as composition and skips option materialization', () => {
    const { runtime, emitted } = createRuntime();

    runtime.noteOptionKeydown('AltLeft');

    expect(runtime.isMacOptionComposition({
      ctrlKey: false,
      metaKey: false,
      key: '@',
    })).toBe(true);
    expect(runtime.shouldMaterializeMacOption({
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      code: 'KeyL',
      key: '@',
    })).toBe(false);

    runtime.handleOptionKeyup('AltLeft');
    expect(emitted).toEqual([]);
  });

  it('materializes option as alt for non-text shortcuts and releases it on keyup', () => {
    const { runtime, emitted } = createRuntime();

    runtime.noteOptionKeydown('AltLeft');
    expect(runtime.shouldMaterializeMacOption({
      altKey: true,
      ctrlKey: false,
      metaKey: false,
      code: 'ArrowLeft',
      key: 'ArrowLeft',
    })).toBe(true);

    runtime.materializeMacOption();
    runtime.handleOptionKeyup('AltLeft');

    expect(emitted).toEqual([
      { code: 'AltLeft', down: true },
      { code: 'AltLeft', down: false },
    ]);
  });
});
