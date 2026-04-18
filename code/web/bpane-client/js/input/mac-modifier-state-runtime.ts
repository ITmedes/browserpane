const MAC_META_TO_CTRL: Record<string, string> = {
  MetaLeft: 'ControlLeft',
  MetaRight: 'ControlRight',
};

const MAC_OPTION_CODES = new Set(['AltLeft', 'AltRight']);
const KEYBOARD_MODIFIER_CODES = new Set([
  'ShiftLeft', 'ShiftRight',
  'ControlLeft', 'ControlRight',
  'AltLeft', 'AltRight',
  'MetaLeft', 'MetaRight',
]);

type MacModifierEmitter = (code: string, down: boolean) => void;

export interface MacModifierCompositionEvent {
  ctrlKey: boolean;
  metaKey: boolean;
  key: string;
}

export interface MacCtrlMaterializeEvent {
  metaKey: boolean;
  code: string;
}

export interface MacOptionMaterializeEvent extends MacModifierCompositionEvent {
  altKey: boolean;
  code: string;
}

export interface MacModifierStateRuntimeInput {
  isMac: boolean;
  macMetaAsCtrl: boolean;
  emitModifierKey: MacModifierEmitter;
}

export class MacModifierStateRuntime {
  private readonly isMac: boolean;
  private readonly macMetaAsCtrl: boolean;
  private readonly emitModifierKey: MacModifierEmitter;
  private readonly activeMacMetaCodes = new Set<string>();
  private readonly materializedMacCtrlCodes = new Set<string>();
  private readonly activeMacOptionCodes = new Set<string>();
  private readonly materializedMacOptionCodes = new Set<string>();

  constructor(input: MacModifierStateRuntimeInput) {
    this.isMac = input.isMac;
    this.macMetaAsCtrl = input.macMetaAsCtrl;
    this.emitModifierKey = input.emitModifierKey;
  }

  isMacMetaKey(code: string): boolean {
    return this.macMetaAsCtrl && Object.hasOwn(MAC_META_TO_CTRL, code);
  }

  isMacOptionKey(code: string): boolean {
    return this.isMac && MAC_OPTION_CODES.has(code);
  }

  noteMetaKeydown(code: string): void {
    if (this.isMacMetaKey(code)) {
      this.activeMacMetaCodes.add(code);
    }
  }

  noteOptionKeydown(code: string): void {
    if (this.isMacOptionKey(code)) {
      this.activeMacOptionCodes.add(code);
    }
  }

  handleMetaKeyup(code: string): boolean {
    if (!this.isMacMetaKey(code)) {
      return false;
    }

    this.activeMacMetaCodes.delete(code);
    const ctrlCode = MAC_META_TO_CTRL[code];
    if (ctrlCode && this.materializedMacCtrlCodes.has(ctrlCode)) {
      this.materializedMacCtrlCodes.delete(ctrlCode);
      this.emitModifierKey(ctrlCode, false);
    }
    return true;
  }

  handleOptionKeyup(code: string): boolean {
    if (!this.isMacOptionKey(code)) {
      return false;
    }

    this.activeMacOptionCodes.delete(code);
    if (this.materializedMacOptionCodes.has(code)) {
      this.materializedMacOptionCodes.delete(code);
      this.emitModifierKey(code, false);
    }
    return true;
  }

  isMacOptionComposition(event: MacModifierCompositionEvent): boolean {
    return this.isMac
      && this.activeMacOptionCodes.size > 0
      && !event.ctrlKey
      && !event.metaKey
      && (event.key === 'Dead' || event.key.length === 1);
  }

  shouldMaterializeMacCtrl(event: MacCtrlMaterializeEvent): boolean {
    return this.macMetaAsCtrl
      && event.metaKey
      && !KEYBOARD_MODIFIER_CODES.has(event.code)
      && this.activeMacMetaCodes.size > 0;
  }

  shouldMaterializeMacOption(event: MacOptionMaterializeEvent): boolean {
    return this.isMac
      && event.altKey
      && !event.ctrlKey
      && !event.metaKey
      && !KEYBOARD_MODIFIER_CODES.has(event.code)
      && this.activeMacOptionCodes.size > 0
      && !this.isMacOptionComposition(event);
  }

  materializeMacCtrl(): void {
    for (const metaCode of this.activeMacMetaCodes) {
      const ctrlCode = MAC_META_TO_CTRL[metaCode];
      if (!ctrlCode || this.materializedMacCtrlCodes.has(ctrlCode)) {
        continue;
      }
      this.emitModifierKey(ctrlCode, true);
      this.materializedMacCtrlCodes.add(ctrlCode);
    }
  }

  materializeMacOption(): void {
    for (const optionCode of this.activeMacOptionCodes) {
      if (this.materializedMacOptionCodes.has(optionCode)) {
        continue;
      }
      this.emitModifierKey(optionCode, true);
      this.materializedMacOptionCodes.add(optionCode);
    }
  }

  releaseMacCtrlsForRemap(): string[] {
    const released: string[] = [];
    for (const metaCode of this.activeMacMetaCodes) {
      const ctrlCode = MAC_META_TO_CTRL[metaCode];
      if (!ctrlCode || !this.materializedMacCtrlCodes.has(ctrlCode)) {
        continue;
      }
      this.materializedMacCtrlCodes.delete(ctrlCode);
      this.emitModifierKey(ctrlCode, false);
      released.push(ctrlCode);
    }
    return released;
  }

  restoreMacCtrls(ctrlCodes: Iterable<string>): void {
    for (const ctrlCode of ctrlCodes) {
      const metaCode = Object.entries(MAC_META_TO_CTRL).find(([, mappedCtrl]) => mappedCtrl === ctrlCode)?.[0];
      if (!metaCode || !this.activeMacMetaCodes.has(metaCode) || this.materializedMacCtrlCodes.has(ctrlCode)) {
        continue;
      }
      this.emitModifierKey(ctrlCode, true);
      this.materializedMacCtrlCodes.add(ctrlCode);
    }
  }

  preferredMacCtrlCode(): string {
    for (const metaCode of this.activeMacMetaCodes) {
      const ctrlCode = MAC_META_TO_CTRL[metaCode];
      if (ctrlCode) {
        return ctrlCode;
      }
    }
    return 'ControlLeft';
  }

  reset(): void {
    this.activeMacMetaCodes.clear();
    this.materializedMacCtrlCodes.clear();
    this.activeMacOptionCodes.clear();
    this.materializedMacOptionCodes.clear();
  }
}
