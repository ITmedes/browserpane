import {
  composeSyntheticDeadAccent,
  getSyntheticDeadAccentSpacingCharacter,
  type SupportedDeadAccent,
} from './synthetic-dead-accent.js';

export interface SyntheticDeadAccentKeyEvent {
  code: string;
  key: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export interface SyntheticDeadAccentKeydownFallback {
  deadCode: string;
  spacingAccent: string;
  deadKeyCode: string | null;
}

export interface SyntheticDeadAccentKeydownResult {
  handled: boolean;
  clearKeyboardSink?: boolean;
  fallback?: SyntheticDeadAccentKeydownFallback;
}

export interface SyntheticDeadAccentKeyupResult {
  handled: boolean;
  emitCharacter?: {
    code: string;
    key: string;
  };
  clearDeadKeyCode: boolean;
}

interface PendingSyntheticDeadAccentState {
  accent: SupportedDeadAccent;
  deadCode: string;
  deadReleased: boolean;
  baseCode: string | null;
  baseChar: string | null;
  baseReleased: boolean;
  emitted: boolean;
}

export class SyntheticDeadAccentRuntime {
  private pending: PendingSyntheticDeadAccentState | null = null;

  begin(accent: SupportedDeadAccent, deadCode: string): void {
    this.pending = {
      accent,
      deadCode,
      deadReleased: false,
      baseCode: null,
      baseChar: null,
      baseReleased: false,
      emitted: false,
    };
  }

  handleKeydown(event: SyntheticDeadAccentKeyEvent): SyntheticDeadAccentKeydownResult {
    const pending = this.pending;
    if (!pending) {
      return { handled: false };
    }

    if (event.code === pending.baseCode) {
      return {
        handled: true,
        clearKeyboardSink: false,
      };
    }

    const composedChar = composeSyntheticDeadAccent(pending.accent, event);
    if (composedChar) {
      pending.baseCode = event.code;
      pending.baseChar = composedChar;
      return {
        handled: true,
        clearKeyboardSink: true,
      };
    }

    if (event.code === pending.deadCode) {
      return {
        handled: true,
        clearKeyboardSink: false,
      };
    }

    this.pending = null;
    return {
      handled: false,
      fallback: {
        deadCode: pending.deadCode,
        spacingAccent: getSyntheticDeadAccentSpacingCharacter(pending.accent),
        deadKeyCode: pending.deadReleased ? null : pending.deadCode,
      },
    };
  }

  handleKeyup(code: string): SyntheticDeadAccentKeyupResult {
    const pending = this.pending;
    if (!pending) {
      return {
        handled: false,
        clearDeadKeyCode: false,
      };
    }

    if (code === pending.baseCode) {
      let emitCharacter: { code: string; key: string } | undefined;
      if (!pending.emitted && pending.baseChar) {
        emitCharacter = {
          code,
          key: pending.baseChar,
        };
        pending.emitted = true;
      }

      if (pending.emitted) {
        pending.baseReleased = true;
        if (pending.deadReleased || pending.deadCode === pending.baseCode) {
          this.pending = null;
          return {
            handled: true,
            emitCharacter,
            clearDeadKeyCode: true,
          };
        }
      }

      return {
        handled: true,
        emitCharacter,
        clearDeadKeyCode: false,
      };
    }

    if (code === pending.deadCode) {
      pending.deadReleased = true;
      if (pending.emitted && pending.baseReleased) {
        this.pending = null;
        return {
          handled: true,
          clearDeadKeyCode: true,
        };
      }

      return {
        handled: true,
        clearDeadKeyCode: false,
      };
    }

    return {
      handled: false,
      clearDeadKeyCode: false,
    };
  }

  reset(): void {
    this.pending = null;
  }
}
