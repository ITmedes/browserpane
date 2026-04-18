export interface DeferredCtrlPasteEvent {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export interface DeferredCtrlPasteRuntimeInput {
  emitKeyEvent: (code: string, down: boolean, ctrl: boolean) => void;
}

interface PendingCtrlPasteState {
  code: string;
  heldCtrlCodes: Set<string>;
  releasedCtrlCodes: Set<string>;
}

export class DeferredCtrlPasteRuntime {
  private readonly emitKeyEvent: (code: string, down: boolean, ctrl: boolean) => void;
  private readonly activeControlCodes = new Set<string>();
  private pending: PendingCtrlPasteState | null = null;

  constructor(input: DeferredCtrlPasteRuntimeInput) {
    this.emitKeyEvent = input.emitKeyEvent;
  }

  noteControlKeydown(code: string): void {
    this.activeControlCodes.add(code);
  }

  noteControlKeyup(code: string): boolean {
    if (this.pending?.heldCtrlCodes.has(code)) {
      this.activeControlCodes.delete(code);
      this.pending.releasedCtrlCodes.add(code);
      return true;
    }

    this.activeControlCodes.delete(code);
    return false;
  }

  shouldDeferPaste(event: DeferredCtrlPasteEvent, clipboardEnabled: boolean): boolean {
    return clipboardEnabled
      && event.code === 'KeyV'
      && event.ctrlKey
      && !event.altKey
      && !event.metaKey
      && !event.shiftKey
      && this.activeControlCodes.size > 0;
  }

  begin(code: string): boolean {
    if (this.pending || this.activeControlCodes.size === 0) {
      return false;
    }

    this.pending = {
      code,
      heldCtrlCodes: new Set(this.activeControlCodes),
      releasedCtrlCodes: new Set<string>(),
    };
    return true;
  }

  flush(): void {
    const pending = this.pending;
    if (!pending) {
      return;
    }

    this.pending = null;
    this.emitKeyEvent(pending.code, true, true);
    this.emitKeyEvent(pending.code, false, true);

    for (const ctrlCode of pending.releasedCtrlCodes) {
      this.emitKeyEvent(ctrlCode, false, false);
    }
  }

  reset(): void {
    this.activeControlCodes.clear();
    this.pending = null;
  }
}
