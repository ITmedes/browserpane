/**
 * InputController — extracted from BpaneSession.
 *
 * Manages DOM event listeners for mouse, keyboard, scroll, clipboard,
 * and encodes/sends input messages over the wire protocol.
 */

import {
  encodeFrame,
  CH_INPUT, CH_CLIPBOARD, CH_CONTROL,
} from './protocol.js';
import { isMacPlatform } from './input-map.js';
import {
  encodeClipboardTextMessage,
  encodeKeyEventMessage,
  encodeLayoutHintMessage,
  encodeMouseButtonMessage,
  encodeMouseMoveMessage,
  encodeScrollMessage,
} from './input/input-message-codec.js';
import { ClipboardSyncRuntime } from './input/clipboard-sync-runtime.js';
import { DeferredCtrlPasteRuntime } from './input/deferred-ctrl-paste-runtime.js';
import { KeyboardSinkRuntime } from './input/keyboard-sink-runtime.js';
import {
  inferLayoutHint,
  inferLayoutName,
  sendKeyboardLayoutHint,
} from './input/layout-hint.js';
import { PointerInputRuntime } from './input/pointer-input-runtime.js';
import { ShortcutGatingPolicy } from './input/shortcut-gating-policy.js';
import { ShortcutKeyReleaseRuntime } from './input/shortcut-key-release-runtime.js';
import { SuppressedKeyupTracker } from './input/suppressed-keyup-tracker.js';
import {
  composeSyntheticDeadAccent,
  getSyntheticDeadAccentSpacingCharacter,
  resolveSupportedDeadAccent,
  type SupportedDeadAccent,
} from './input/synthetic-dead-accent.js';
import { MacModifierStateRuntime } from './input/mac-modifier-state-runtime.js';
import { PendingCompositionRuntime } from './input/pending-composition-runtime.js';
import { fnvHash } from './hash.js';

const PENDING_COMPOSITION_FALLBACK_MS = 16;
const SUPPRESSED_KEYUP_TIMEOUT_MS = 750;

const CTRL_KEY_CODES = new Set(['ControlLeft', 'ControlRight']);

interface RemappedKeyState {
  releasedCtrlCodes: string[];
}

interface PendingSyntheticAccentState {
  accent: SupportedDeadAccent;
  deadCode: string;
  deadReleased: boolean;
  baseCode: string | null;
  baseChar: string | null;
  baseReleased: boolean;
  emitted: boolean;
}

export interface InputControllerDeps {
  /** Canvas element to bind event listeners to. */
  canvas: HTMLCanvasElement;
  /** Send a protocol frame. */
  sendFrame: (channelId: number, payload: Uint8Array) => void;
  /** Draw remote cursor at given coordinates. */
  drawCursor: (shape: null, x: number, y: number) => void;
  /** Get current remote dimensions for coordinate mapping. */
  getRemoteDims: () => { width: number; height: number };
  /** Whether clipboard syncing is enabled. */
  clipboardEnabled: boolean;
  /** Remap Mac Command to Ctrl. Default: true on Mac. */
  macMetaAsCtrl?: boolean;
}

export class InputController {
  private canvas: HTMLCanvasElement;
  private sendFrame: (channelId: number, payload: Uint8Array) => void;
  private drawCursor: (shape: null, x: number, y: number) => void;
  private getRemoteDims: () => { width: number; height: number };
  private clipboardEnabled: boolean;
  private macMetaAsCtrl: boolean;
  private isMac: boolean;

  private deadKeyPending = false;
  private deadKeyCode: string | null = null;
  private inputAbortController: AbortController | null = null;
  private lastClipboardHash: bigint = 0n;
  private readonly clipboardSync: ClipboardSyncRuntime;
  private readonly keyboardSink: KeyboardSinkRuntime;
  private readonly pointerInput: PointerInputRuntime;
  private readonly suppressedKeyups: SuppressedKeyupTracker;
  /** Tracks keys remapped on keydown (e.g., ArrowLeft→Home) for correct keyup. */
  private remappedKeys = new Map<string, RemappedKeyState>();
  /** Narrow mac dead-key workaround for accented vowels. */
  private pendingSyntheticAccent: PendingSyntheticAccentState | null = null;
  private readonly pendingComposition: PendingCompositionRuntime;
  private readonly macModifiers: MacModifierStateRuntime;
  private readonly deferredCtrlPaste: DeferredCtrlPasteRuntime;
  private readonly shortcutPolicy: ShortcutGatingPolicy;
  private readonly shortcutKeyRelease: ShortcutKeyReleaseRuntime;

  // Set by BpaneSession when server capabilities are received
  serverSupportsKeyEventEx = false;

  constructor(deps: InputControllerDeps) {
    this.canvas = deps.canvas;
    this.sendFrame = deps.sendFrame;
    this.drawCursor = deps.drawCursor;
    this.getRemoteDims = deps.getRemoteDims;
    this.clipboardEnabled = deps.clipboardEnabled;
    this.isMac = isMacPlatform();
    this.macMetaAsCtrl = deps.macMetaAsCtrl ?? this.isMac;
    this.clipboardSync = new ClipboardSyncRuntime({
      canvas: this.canvas,
      sendClipboardText: (text) => {
        this.sendClipboardText(text);
      },
      getLastClipboardHash: () => this.lastClipboardHash,
      setLastClipboardHash: (hash) => {
        this.lastClipboardHash = hash;
      },
      navigatorLike: typeof navigator === 'undefined' ? undefined : navigator,
      documentLike: typeof document === 'undefined' ? undefined : document,
    });
    this.pointerInput = new PointerInputRuntime({
      canvas: this.canvas,
      drawCursor: this.drawCursor,
      getRemoteDims: this.getRemoteDims,
      sendMouseMove: (x, y) => {
        this.sendMouseMove(x, y);
      },
      sendMouseButton: (button, down, x, y) => {
        this.sendMouseButton(button, down, x, y);
      },
      sendScroll: (dx, dy) => {
        this.sendScroll(dx, dy);
      },
    });
    this.keyboardSink = new KeyboardSinkRuntime({
      canvas: this.canvas,
      documentLike: typeof document === 'undefined' ? undefined : document,
    });
    this.suppressedKeyups = new SuppressedKeyupTracker({
      timeoutMs: SUPPRESSED_KEYUP_TIMEOUT_MS,
      setTimeoutFn: window.setTimeout,
      clearTimeoutFn: window.clearTimeout,
    });
    this.deferredCtrlPaste = new DeferredCtrlPasteRuntime({
      emitKeyEvent: (code, down, ctrl) => {
        this.sendKeyEvent(code, '', down, ctrl, false, false, false, false);
      },
    });
    this.shortcutPolicy = new ShortcutGatingPolicy({
      isMac: this.isMac,
      macMetaAsCtrl: this.macMetaAsCtrl,
    });
    this.shortcutKeyRelease = new ShortcutKeyReleaseRuntime();
    this.macModifiers = new MacModifierStateRuntime({
      isMac: this.isMac,
      macMetaAsCtrl: this.macMetaAsCtrl,
      emitModifierKey: (code, down) => {
        this.sendKeyEvent(code, '', down, false, false, false, false, false);
      },
    });
    this.pendingComposition = new PendingCompositionRuntime({
      fallbackDelayMs: PENDING_COMPOSITION_FALLBACK_MS,
      setTimeoutFn: window.setTimeout,
      clearTimeoutFn: window.clearTimeout,
      emitCharacter: (input) => {
        this.sendKeyEvent(input.code, input.key, true, false, false, input.shift, false, false);
        this.sendKeyEvent(input.code, input.key, false, false, false, input.shift, false, false);
      },
      suppressKeyup: (code) => {
        this.suppressedKeyups.suppress(code);
      },
      clearKeyboardSink: () => {
        this.keyboardSink.clear();
      },
    });
  }

  /** Set up all DOM event listeners on the canvas. */
  setup(): void {
    this.inputAbortController = new AbortController();
    const signal = this.inputAbortController.signal;
    const keyboardTarget = this.keyboardSink.ensure();
    const handleCompositionEnd = (e: Event) => {
      const event = e as CompositionEvent;
      this.pendingComposition.commit(event.data ?? '');
      this.keyboardSink.clear();
    };
    const handleTextInput = (e: Event) => {
      const event = e as InputEvent;
      const text = event.data ?? this.keyboardSink.getValue();
      this.pendingComposition.commit(text);
      this.keyboardSink.clear();
    };
    this.pointerInput.bind({
      signal,
      focusKeyboardTarget: () => {
        keyboardTarget.focus();
      },
    });

    keyboardTarget.addEventListener('keydown', (e: KeyboardEvent) => {
      if (!e.repeat) {
        this.suppressedKeyups.clear(e.code);
      }

      if (this.macModifiers.isMacMetaKey(e.code)) {
        e.preventDefault();
        this.macModifiers.noteMetaKeydown(e.code);
        return;
      }

      if (this.macModifiers.isMacOptionKey(e.code)) {
        e.preventDefault();
        this.macModifiers.noteOptionKeydown(e.code);
        return;
      }

      if (CTRL_KEY_CODES.has(e.code)) {
        this.deferredCtrlPaste.noteControlKeydown(e.code);
      }

      if (this.remappedKeys.has(e.code)) {
        e.preventDefault();
        return;
      }

      const supportedDeadAccent = resolveSupportedDeadAccent(e, this.isMac);
      if (supportedDeadAccent) {
        e.preventDefault();
        this.deadKeyPending = false;
        this.pendingComposition.reset();
        this.keyboardSink.clear();
        this.deadKeyCode = e.code;
        this.pendingSyntheticAccent = {
          accent: supportedDeadAccent,
          deadCode: e.code,
          deadReleased: false,
          baseCode: null,
          baseChar: null,
          baseReleased: false,
          emitted: false,
        };
        return;
      }

      if (this.pendingSyntheticAccent) {
        if (e.code === this.pendingSyntheticAccent.baseCode) {
          e.preventDefault();
          return;
        }

        const composedChar = composeSyntheticDeadAccent(this.pendingSyntheticAccent.accent, e);
        if (composedChar) {
          e.preventDefault();
          this.pendingSyntheticAccent.baseCode = e.code;
          this.pendingSyntheticAccent.baseChar = composedChar;
          this.keyboardSink.clear();
          return;
        }

        if (e.code === this.pendingSyntheticAccent.deadCode) {
          e.preventDefault();
          return;
        }

        this.emitSyntheticAccentFallback(this.pendingSyntheticAccent);
        this.pendingSyntheticAccent = null;
      }

      if (e.isComposing && !this.deadKeyPending) return;

      // Keep the hosted Chromium window pinned open and at a fixed size.
      if (this.shortcutPolicy.shouldSuppressLockedWindowShortcut(e)) {
        e.preventDefault();
        if (!e.repeat) {
          this.suppressedKeyups.suppress(e.code);
        }
        return;
      }

      // Mac Meta passthrough: let browser handle Cmd+Q, Cmd+W, Cmd+Tab
      if (this.shortcutPolicy.shouldPassThroughMacMetaShortcut(e)) {
        this.macModifiers.releaseMacCtrlsForRemap();
        return; // don't preventDefault, let browser handle
      }

      const effectiveCtrl = e.ctrlKey || (this.macMetaAsCtrl && e.metaKey);
      if (this.shortcutPolicy.shouldSendAtomicMacCtrlShortcut(e)) {
        e.preventDefault();
        if (e.repeat) {
          return;
        }
        this.suppressedKeyups.suppress(e.code);
        this.dispatchAtomicMacCtrlShortcutWithClipboardSync(e.code, e.key);
        return;
      }

      if (this.shouldDeferCtrlPasteShortcut(e)) {
        e.preventDefault();
        if (e.repeat || !this.deferredCtrlPaste.begin(e.code)) {
          return;
        }
        this.suppressedKeyups.suppress(e.code);
        void this.syncClipboardBeforePaste().finally(() => {
          this.flushDeferredCtrlPaste();
        });
        return;
      }

      // Intercept Ctrl+V / Cmd+V: sync clipboard before keystroke
      if (effectiveCtrl && e.code === 'KeyV' && !e.repeat && this.clipboardEnabled) {
        navigator.clipboard.readText().then((text) => {
          if (text) {
            const hash = fnvHash(text);
            if (hash !== this.lastClipboardHash) {
              this.lastClipboardHash = hash;
              this.sendClipboardText(text);
            }
          }
        }).catch(() => {});
      }

      // Mac Cmd+Arrow → Home/End remapping (text line selection shortcuts).
      // Dispatch atomically on keydown so later modifier release/repeat events
      // cannot collapse the selection by reusing a held synthetic Home/End key.
      if (this.macMetaAsCtrl && e.metaKey && (e.code === 'ArrowLeft' || e.code === 'ArrowRight')) {
        e.preventDefault();
        const remapCode = e.code === 'ArrowLeft' ? 'Home' : 'End';
        const releasedCtrlCodes = this.macModifiers.releaseMacCtrlsForRemap();
        this.remappedKeys.set(e.code, { releasedCtrlCodes });
        this.sendKeyEvent(remapCode, '', true, false, false, e.shiftKey, false, false);
        this.sendKeyEvent(remapCode, '', false, false, false, e.shiftKey, false, false);
        this.macModifiers.restoreMacCtrls(releasedCtrlCodes);
        return;
      }

      // Dead keys: do NOT call preventDefault — browser needs default handling
      // to enter composition mode. Without it, ´+e won't produce é.
      if (e.key === 'Dead') {
        this.deadKeyPending = true;
        this.deadKeyCode = e.code;
        return;
      }

      if (this.deadKeyPending) {
        this.deadKeyPending = false;
        this.pendingComposition.begin({
          code: e.code,
          shift: e.shiftKey,
          fallbackKey: e.key,
        });
        return;
      }

      if (this.pendingComposition.hasPendingCode(e.code)) {
        e.preventDefault();
        return;
      }

      e.preventDefault();

      if (this.deadKeyPending) {
        this.deadKeyPending = false;
        // Keep deadKeyCode set — we need it to suppress the dead key's own keyup
      }

      if (this.macModifiers.shouldMaterializeMacCtrl(e)) {
        this.macModifiers.materializeMacCtrl();
      }
      if (this.macModifiers.shouldMaterializeMacOption(e)) {
        this.macModifiers.materializeMacOption();
      }

      // Detect AltGr: getModifierState('AltGraph') is true (Windows/Linux)
      let altgr = e.getModifierState('AltGraph');

      // Compute effective modifier state
      let ctrl = e.ctrlKey;
      let alt = e.altKey;
      const meta = e.metaKey;

      // Mac: remap Meta → Ctrl (Command key sends Ctrl to Linux)
      if (this.macMetaAsCtrl && meta) {
        ctrl = true;
      }

      // AltGr (Windows/Linux): set ALT but NOT CTRL
      // Windows sends fake Ctrl+Alt for AltGr — strip the fake Ctrl
      if (altgr) {
        ctrl = false;
        alt = true;
      }

      // Mac Option composition: Option+key producing a printable character
      // is character composition (like AltGr), not a raw Alt modifier.
      // This handles @, |, \, ~, {, }, [, ] and dead-key sequences on macOS.
      if (this.macModifiers.isMacOptionComposition(e)) {
        altgr = true;
        alt = false;
      }

      if (this.sendKeyEvent(e.code, e.key, true, ctrl, alt, e.shiftKey, meta && !this.macMetaAsCtrl, altgr)) {
        this.shortcutKeyRelease.noteSentKeydown({
          code: e.code,
          key: e.key,
          ctrl,
          alt,
          shift: e.shiftKey,
          meta: meta && !this.macMetaAsCtrl,
          altgr,
        });
      }
    }, { signal });

    keyboardTarget.addEventListener('keyup', (e: KeyboardEvent) => {
      // Clear remapped keys (e.g., Cmd+Left was sent as atomic Home)
      const remapped = this.remappedKeys.get(e.code);
      if (remapped) {
        e.preventDefault();
        this.remappedKeys.delete(e.code);
        return;
      }

      if (this.macModifiers.isMacMetaKey(e.code)) {
        this.releaseTrackedShortcutKeysForModifierKeyup(e);
        this.macModifiers.handleMetaKeyup(e.code);
        e.preventDefault();
        return;
      }

      if (this.macModifiers.isMacOptionKey(e.code)) {
        this.releaseTrackedShortcutKeysForModifierKeyup(e);
        this.macModifiers.handleOptionKeyup(e.code);
        e.preventDefault();
        return;
      }

      if (CTRL_KEY_CODES.has(e.code)) {
        if (this.deferredCtrlPaste.noteControlKeyup(e.code)) {
          e.preventDefault();
          return;
        }
      }

      this.shortcutKeyRelease.noteObservedKeyup(e.code);

      if (this.pendingSyntheticAccent) {
        if (e.code === this.pendingSyntheticAccent.baseCode) {
          e.preventDefault();
          if (!this.pendingSyntheticAccent.emitted && this.pendingSyntheticAccent.baseChar) {
            this.sendKeyEvent(e.code, this.pendingSyntheticAccent.baseChar, true, false, false, false, false, false);
            this.sendKeyEvent(e.code, this.pendingSyntheticAccent.baseChar, false, false, false, false, false, false);
            this.pendingSyntheticAccent.emitted = true;
          }
          if (this.pendingSyntheticAccent.emitted) {
            this.pendingSyntheticAccent.baseReleased = true;
            if (this.pendingSyntheticAccent.deadReleased
              || this.pendingSyntheticAccent.deadCode === this.pendingSyntheticAccent.baseCode) {
              this.pendingSyntheticAccent = null;
              this.deadKeyCode = null;
            }
          }
          return;
        }
        if (e.code === this.pendingSyntheticAccent.deadCode) {
          e.preventDefault();
          this.pendingSyntheticAccent.deadReleased = true;
          if (this.pendingSyntheticAccent.emitted && this.pendingSyntheticAccent.baseReleased) {
            this.pendingSyntheticAccent = null;
            this.deadKeyCode = null;
          }
          return;
        }
      }

      if (this.suppressedKeyups.clear(e.code)) {
        e.preventDefault();
        return;
      }

      if (this.pendingComposition.handleKeyup(e.code)) {
        e.preventDefault();
        return;
      }

      if (e.isComposing) return;

      // Suppress the dead key's own keyup (unpaired release prevention)
      if (e.code === this.deadKeyCode) {
        this.deadKeyCode = null;
        return;
      }

      this.releaseTrackedShortcutKeysForModifierKeyup(e);
      e.preventDefault();

      let altgr = e.getModifierState('AltGraph');

      let ctrl = e.ctrlKey;
      let alt = e.altKey;
      const meta = e.metaKey;

      if (this.macMetaAsCtrl && meta) {
        ctrl = true;
      }

      if (altgr) {
        ctrl = false;
        alt = true;
      }

      // Mac Option composition on keyup (same detection as keydown)
      if (this.macModifiers.isMacOptionComposition(e)) {
        altgr = true;
        alt = false;
      }

      this.sendKeyEvent(e.code, e.key, false, ctrl, alt, e.shiftKey, meta && !this.macMetaAsCtrl, altgr);
    }, { signal });

    keyboardTarget.addEventListener('compositionend', handleCompositionEnd, { signal });
    keyboardTarget.addEventListener('input', handleTextInput, { signal });
    document.addEventListener('compositionend', handleCompositionEnd, { capture: true, signal });

    if (this.clipboardEnabled) {
      this.clipboardSync.bind({
        keyboardTarget,
        signal,
      });
    }
  }

  /** Remove all DOM event listeners. */
  destroy(): void {
    if (this.inputAbortController) {
      this.inputAbortController.abort();
      this.inputAbortController = null;
    }
    this.pointerInput.reset();
    this.clipboardSync.reset();
    this.deferredCtrlPaste.reset();
    this.macModifiers.reset();
    this.remappedKeys.clear();
    this.pendingComposition.reset();
    this.pendingSyntheticAccent = null;
    this.suppressedKeyups.reset();
    this.shortcutKeyRelease.reset();
    this.keyboardSink.destroy();
  }

  /** Update the clipboard hash when a remote clipboard message arrives. */
  setLastClipboardHash(hash: bigint): void {
    this.lastClipboardHash = hash;
  }

  sendClipboardText(text: string): void {
    this.sendFrame(CH_CLIPBOARD, encodeClipboardTextMessage(text));
  }

  // ── Input message encoding ─────────────────────────────────────────

  private sendMouseMove(x: number, y: number): void {
    this.sendFrame(CH_INPUT, encodeMouseMoveMessage(x, y));
  }

  private sendMouseButton(button: number, down: boolean, x: number, y: number): void {
    this.sendFrame(CH_INPUT, encodeMouseButtonMessage(button, down, x, y));
  }

  private sendScroll(dx: number, dy: number): void {
    this.sendFrame(CH_INPUT, encodeScrollMessage(dx, dy));
  }

  private sendKeyEvent(
    code: string,
    key: string,
    down: boolean,
    ctrl: boolean,
    alt: boolean,
    shift: boolean,
    meta: boolean,
    altgr: boolean = false,
  ): boolean {
    const payload = encodeKeyEventMessage({
      code,
      key,
      down,
      ctrl,
      alt,
      shift,
      meta,
      altgr,
      extended: this.serverSupportsKeyEventEx,
    });
    if (!payload) {
      return false;
    }
    this.sendFrame(CH_INPUT, payload);
    return true;
  }

  /** Send keyboard layout hint to server. */
  sendLayoutHint(): void {
    sendKeyboardLayoutHint({
      navigatorLike: typeof navigator === 'undefined' ? undefined : navigator,
      sendHint: (hint) => {
        this.sendFrame(CH_CONTROL, encodeLayoutHintMessage(hint));
      },
    });
  }

  private emitSyntheticAccentFallback(pending: PendingSyntheticAccentState): void {
    const spacingAccent = getSyntheticDeadAccentSpacingCharacter(pending.accent);
    this.sendKeyEvent(pending.deadCode, spacingAccent, true, false, false, false, false, false);
    this.sendKeyEvent(pending.deadCode, spacingAccent, false, false, false, false, false, false);
    if (pending.deadReleased) {
      this.deadKeyCode = null;
    } else {
      this.deadKeyCode = pending.deadCode;
    }
  }

  private shouldDeferCtrlPasteShortcut(e: KeyboardEvent): boolean {
    return this.deferredCtrlPaste.shouldDeferPaste(e, this.clipboardEnabled);
  }

  private dispatchAtomicMacCtrlShortcut(code: string, key: string): void {
    const ctrlCode = this.macModifiers.preferredMacCtrlCode();
    this.sendKeyEvent(ctrlCode, '', true, false, false, false, false, false);
    this.sendKeyEvent(code, key, true, true, false, false, false, false);
    this.sendKeyEvent(code, key, false, true, false, false, false, false);
    this.sendKeyEvent(ctrlCode, '', false, false, false, false, false, false);
  }

  private dispatchAtomicMacCtrlShortcutWithClipboardSync(code: string, key: string): void {
    if (code !== 'KeyV' || !this.clipboardEnabled) {
      this.dispatchAtomicMacCtrlShortcut(code, key);
      return;
    }

    void this.syncClipboardBeforePaste().finally(() => {
      this.dispatchAtomicMacCtrlShortcut(code, key);
    });
  }

  private syncClipboardBeforePaste(): Promise<void> {
    return this.clipboardSync.syncClipboardBeforePaste();
  }

  private flushDeferredCtrlPaste(): void {
    this.deferredCtrlPaste.flush();
  }

  private releaseTrackedShortcutKeysForModifierKeyup(e: KeyboardEvent): void {
    const releases = this.shortcutKeyRelease.releaseKeysForModifierKeyup(e);
    for (const release of releases) {
      this.sendKeyEvent(
        release.code,
        release.key,
        false,
        release.ctrl,
        release.alt,
        release.shift,
        release.meta,
        release.altgr,
      );
      this.suppressedKeyups.suppress(release.code);
    }
  }
}

export { inferLayoutName, inferLayoutHint } from './input/layout-hint.js';
