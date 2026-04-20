const SHORTCUT_MODIFIER_CODES = new Set([
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'MetaLeft',
  'MetaRight',
]);

export interface ShortcutReleasedKey {
  code: string;
  key: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  altgr: boolean;
}

export interface ShortcutModifierKeyupEvent {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

export class ShortcutKeyReleaseRuntime {
  private readonly trackedKeys = new Map<string, ShortcutReleasedKey>();

  noteSentKeydown(input: ShortcutReleasedKey): void {
    if (SHORTCUT_MODIFIER_CODES.has(input.code)) {
      return;
    }
    if (input.altgr || (!input.ctrl && !input.alt && !input.meta)) {
      return;
    }
    this.trackedKeys.set(input.code, { ...input });
  }

  noteObservedKeyup(code: string): void {
    this.trackedKeys.delete(code);
  }

  releaseKeysForModifierKeyup(input: ShortcutModifierKeyupEvent): ShortcutReleasedKey[] {
    if (!SHORTCUT_MODIFIER_CODES.has(input.code)) {
      return [];
    }
    if (input.ctrlKey || input.altKey || input.metaKey) {
      return [];
    }

    const releases = [...this.trackedKeys.values()];
    this.trackedKeys.clear();
    return releases;
  }

  reset(): void {
    this.trackedKeys.clear();
  }
}
