import type { MacNavigationRemapRuntime } from './mac-navigation-remap-runtime.js';

export interface MacNavigationShortcutDispatcherInput {
  remapRuntime: MacNavigationRemapRuntime;
  releaseMacCtrlsForRemap: () => string[];
  restoreMacCtrls: (ctrlCodes: Iterable<string>) => void;
  emitNavigationKey: (code: 'Home' | 'End', down: boolean, shift: boolean) => void;
}

export class MacNavigationShortcutDispatcher {
  private readonly remapRuntime: MacNavigationRemapRuntime;
  private readonly releaseMacCtrlsForRemap: () => string[];
  private readonly restoreMacCtrls: (ctrlCodes: Iterable<string>) => void;
  private readonly emitNavigationKey: (code: 'Home' | 'End', down: boolean, shift: boolean) => void;

  constructor(input: MacNavigationShortcutDispatcherInput) {
    this.remapRuntime = input.remapRuntime;
    this.releaseMacCtrlsForRemap = input.releaseMacCtrlsForRemap;
    this.restoreMacCtrls = input.restoreMacCtrls;
    this.emitNavigationKey = input.emitNavigationKey;
  }

  dispatchShortcut(code: string, shiftKey: boolean): boolean {
    const releasedCtrlCodes = this.releaseMacCtrlsForRemap();
    const remap = this.remapRuntime.begin(code, releasedCtrlCodes);
    if (!remap) {
      this.restoreMacCtrls(releasedCtrlCodes);
      return false;
    }

    this.emitNavigationKey(remap.remapCode, true, shiftKey);
    this.emitNavigationKey(remap.remapCode, false, shiftKey);
    this.restoreMacCtrls(releasedCtrlCodes);
    return true;
  }
}
