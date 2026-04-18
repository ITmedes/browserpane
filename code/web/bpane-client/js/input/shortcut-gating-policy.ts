interface ShortcutKeyEvent {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export interface ShortcutGatingPolicyInput {
  isMac: boolean;
  macMetaAsCtrl: boolean;
}

export class ShortcutGatingPolicy {
  private readonly isMac: boolean;
  private readonly macMetaAsCtrl: boolean;
  private readonly macMetaPassthrough = new Set(['KeyQ', 'KeyW', 'Tab']);
  private readonly macMetaAtomicShortcuts = new Set(['KeyC', 'KeyV']);

  constructor(input: ShortcutGatingPolicyInput) {
    this.isMac = input.isMac;
    this.macMetaAsCtrl = input.macMetaAsCtrl;
  }

  shouldSuppressLockedWindowShortcut(event: ShortcutKeyEvent): boolean {
    if (!event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey && event.code === 'F11') {
      return true;
    }

    if (!event.ctrlKey && event.altKey && !event.metaKey && !event.shiftKey && event.code === 'F4') {
      return true;
    }

    return !this.isMac
      && event.ctrlKey
      && !event.altKey
      && !event.metaKey
      && (event.code === 'KeyQ' || event.code === 'KeyW');
  }

  shouldPassThroughMacMetaShortcut(event: ShortcutKeyEvent): boolean {
    return this.macMetaAsCtrl
      && event.metaKey
      && this.macMetaPassthrough.has(event.code);
  }

  shouldSendAtomicMacCtrlShortcut(event: ShortcutKeyEvent): boolean {
    return this.macMetaAsCtrl
      && event.metaKey
      && !event.ctrlKey
      && !event.altKey
      && !event.shiftKey
      && this.macMetaAtomicShortcuts.has(event.code);
  }
}
