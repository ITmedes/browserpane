export interface DeadKeyPendingCompositionInput {
  code: string;
  shift: boolean;
  fallbackKey: string;
}

export interface DeadKeySyntheticFallbackInput {
  deadCode: string;
  spacingAccent: string;
  deadKeyCode: string | null;
}

export interface DeadKeyStateRuntimeInput {
  resetPendingComposition: () => void;
  clearKeyboardSink: () => void;
  beginPendingComposition: (input: DeadKeyPendingCompositionInput) => void;
  emitSyntheticKeyEvent: (code: string, key: string, down: boolean) => void;
}

export class DeadKeyStateRuntime {
  private readonly resetPendingComposition: () => void;
  private readonly clearKeyboardSink: () => void;
  private readonly beginPendingComposition: (input: DeadKeyPendingCompositionInput) => void;
  private readonly emitSyntheticKeyEvent: (code: string, key: string, down: boolean) => void;

  private deadKeyPending = false;
  private deadKeyCode: string | null = null;

  constructor(input: DeadKeyStateRuntimeInput) {
    this.resetPendingComposition = input.resetPendingComposition;
    this.clearKeyboardSink = input.clearKeyboardSink;
    this.beginPendingComposition = input.beginPendingComposition;
    this.emitSyntheticKeyEvent = input.emitSyntheticKeyEvent;
  }

  startSupportedDeadAccent(code: string): void {
    this.deadKeyPending = false;
    this.resetPendingComposition();
    this.clearKeyboardSink();
    this.deadKeyCode = code;
  }

  applySyntheticAccentFallback(input: DeadKeySyntheticFallbackInput): void {
    this.emitSyntheticKeyEvent(input.deadCode, input.spacingAccent, true);
    this.emitSyntheticKeyEvent(input.deadCode, input.spacingAccent, false);
    this.deadKeyCode = input.deadKeyCode;
  }

  shouldIgnoreComposingKeydown(isComposing: boolean): boolean {
    return isComposing && !this.deadKeyPending;
  }

  noteNativeDeadKey(code: string): void {
    this.deadKeyPending = true;
    this.deadKeyCode = code;
  }

  beginPendingCompositionIfNeeded(input: DeadKeyPendingCompositionInput): boolean {
    if (!this.deadKeyPending) {
      return false;
    }

    this.deadKeyPending = false;
    this.beginPendingComposition(input);
    return true;
  }

  clearTrackedDeadKey(): void {
    this.deadKeyCode = null;
  }

  consumeTrackedDeadKeyKeyup(code: string): boolean {
    if (code !== this.deadKeyCode) {
      return false;
    }

    this.deadKeyCode = null;
    return true;
  }

  reset(): void {
    this.deadKeyPending = false;
    this.deadKeyCode = null;
  }
}
