export interface AtomicMacShortcutKeyEvent {
  code: string;
  key: string;
  down: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  altgr: boolean;
}

export interface AtomicMacShortcut {
  code: string;
  key: string;
}

export interface AtomicMacPasteShortcut extends AtomicMacShortcut {
  clipboardEnabled: boolean;
}

export interface AtomicMacShortcutDispatcherInput {
  getPreferredCtrlCode: () => string;
  emitKeyEvent: (event: AtomicMacShortcutKeyEvent) => void;
  syncClipboardBeforePaste: () => Promise<void>;
}

export class AtomicMacShortcutDispatcher {
  private readonly getPreferredCtrlCode: () => string;
  private readonly emitKeyEvent: (event: AtomicMacShortcutKeyEvent) => void;
  private readonly syncClipboardBeforePaste: () => Promise<void>;

  constructor(input: AtomicMacShortcutDispatcherInput) {
    this.getPreferredCtrlCode = input.getPreferredCtrlCode;
    this.emitKeyEvent = input.emitKeyEvent;
    this.syncClipboardBeforePaste = input.syncClipboardBeforePaste;
  }

  dispatchShortcut(shortcut: AtomicMacShortcut): void {
    const ctrlCode = this.getPreferredCtrlCode();
    this.emitKeyEvent({
      code: ctrlCode,
      key: '',
      down: true,
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
    this.emitKeyEvent({
      code: shortcut.code,
      key: shortcut.key,
      down: true,
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
    this.emitKeyEvent({
      code: shortcut.code,
      key: shortcut.key,
      down: false,
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
    this.emitKeyEvent({
      code: ctrlCode,
      key: '',
      down: false,
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
    });
  }

  dispatchShortcutWithClipboardSync(shortcut: AtomicMacPasteShortcut): void {
    if (shortcut.code !== 'KeyV' || !shortcut.clipboardEnabled) {
      this.dispatchShortcut(shortcut);
      return;
    }

    const dispatchShortcut = () => {
      this.dispatchShortcut(shortcut);
    };
    void this.syncClipboardBeforePaste().then(dispatchShortcut, dispatchShortcut);
  }
}
