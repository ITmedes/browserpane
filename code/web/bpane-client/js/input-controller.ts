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
import { KeyboardSinkRuntime } from './input/keyboard-sink-runtime.js';
import {
  isMacMetaKey,
  isMacOptionComposition,
  isMacOptionKey,
  shouldDeferCtrlPasteShortcut,
  shouldMaterializeMacCtrl,
  shouldMaterializeMacOption,
  shouldSendAtomicMacCtrlShortcut,
  shouldSuppressLockedWindowShortcut,
} from './input/keyboard-shortcut-policy.js';
import {
  inferLayoutHint,
  inferLayoutName,
  sendKeyboardLayoutHint,
} from './input/layout-hint.js';
import { PointerInputRuntime } from './input/pointer-input-runtime.js';
import { SuppressedKeyupTracker } from './input/suppressed-keyup-tracker.js';
import {
  composeSyntheticDeadAccent,
  getSyntheticDeadAccentSpacingCharacter,
  resolveSupportedDeadAccent,
  type SupportedDeadAccent,
} from './input/synthetic-dead-accent.js';
import { fnvHash } from './hash.js';

const PENDING_COMPOSITION_FALLBACK_MS = 16;
const SUPPRESSED_KEYUP_TIMEOUT_MS = 750;

/** Keys that should NOT be remapped from Meta→Ctrl on Mac (let browser handle). */
const MAC_META_PASSTHROUGH = new Set(['KeyQ', 'KeyW', 'Tab']);
const MAC_META_TO_CTRL: Record<string, string> = {
  MetaLeft: 'ControlLeft',
  MetaRight: 'ControlRight',
};
const CTRL_KEY_CODES = new Set(['ControlLeft', 'ControlRight']);

interface RemappedKeyState {
  releasedCtrlCodes: string[];
}

interface PendingCompositionState {
  code: string;
  shift: boolean;
  fallbackKey: string;
}

interface PendingCtrlPasteState {
  code: string;
  heldCtrlCodes: Set<string>;
  releasedCtrlCodes: Set<string>;
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
  /** Command keys currently held in the browser. */
  private activeMacMetaCodes = new Set<string>();
  /** Physical Control keys currently held in the browser. */
  private activeControlCodes = new Set<string>();
  /** Command keys materialized as Ctrl on the host. */
  private materializedMacCtrlCodes = new Set<string>();
  /** Option keys currently held on macOS. */
  private activeMacOptionCodes = new Set<string>();
  /** Option keys materialized as Alt on the host for shortcut use. */
  private materializedMacOptionCodes = new Set<string>();
  /** Dead-key composition awaiting the final composed character. */
  private pendingComposition: PendingCompositionState | null = null;
  /** Narrow mac dead-key workaround for accented vowels. */
  private pendingSyntheticAccent: PendingSyntheticAccentState | null = null;
  /** Timer used to fall back to the base key if no composed text arrives. */
  private pendingCompositionFallbackTimer: number | null = null;
  /** Deferred Ctrl+V while clipboard sync completes. */
  private pendingCtrlPaste: PendingCtrlPasteState | null = null;

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
  }

  /** Set up all DOM event listeners on the canvas. */
  setup(): void {
    this.inputAbortController = new AbortController();
    const signal = this.inputAbortController.signal;
    const keyboardTarget = this.keyboardSink.ensure();
    const handleCompositionEnd = (e: Event) => {
      const event = e as CompositionEvent;
      this.commitPendingComposition(event.data ?? '');
      this.keyboardSink.clear();
    };
    const handleTextInput = (e: Event) => {
      const event = e as InputEvent;
      const text = event.data ?? this.keyboardSink.getValue();
      this.commitPendingComposition(text);
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

      if (isMacMetaKey(e.code, this.macMetaAsCtrl)) {
        e.preventDefault();
        this.activeMacMetaCodes.add(e.code);
        return;
      }

      if (isMacOptionKey(e.code, this.isMac)) {
        e.preventDefault();
        this.activeMacOptionCodes.add(e.code);
        return;
      }

      if (CTRL_KEY_CODES.has(e.code)) {
        this.activeControlCodes.add(e.code);
      }

      if (this.remappedKeys.has(e.code)) {
        e.preventDefault();
        return;
      }

      const supportedDeadAccent = resolveSupportedDeadAccent(e, this.isMac);
      if (supportedDeadAccent) {
        e.preventDefault();
        this.deadKeyPending = false;
        this.pendingComposition = null;
        this.clearPendingCompositionFallback();
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
      if (shouldSuppressLockedWindowShortcut(e, { isMac: this.isMac })) {
        e.preventDefault();
        if (!e.repeat) {
          this.suppressedKeyups.suppress(e.code);
        }
        return;
      }

      // Mac Meta passthrough: let browser handle Cmd+Q, Cmd+W, Cmd+Tab
      if (this.macMetaAsCtrl && e.metaKey && MAC_META_PASSTHROUGH.has(e.code)) {
        this.releaseMacCtrlsForRemap();
        return; // don't preventDefault, let browser handle
      }

      const effectiveCtrl = e.ctrlKey || (this.macMetaAsCtrl && e.metaKey);
      if (shouldSendAtomicMacCtrlShortcut(e, { macMetaAsCtrl: this.macMetaAsCtrl })) {
        e.preventDefault();
        if (e.repeat) {
          return;
        }
        this.suppressedKeyups.suppress(e.code);
        this.dispatchAtomicMacCtrlShortcutWithClipboardSync(e.code, e.key);
        return;
      }

      if (shouldDeferCtrlPasteShortcut(e, {
        clipboardEnabled: this.clipboardEnabled,
        activeControlCount: this.activeControlCodes.size,
      })) {
        e.preventDefault();
        if (e.repeat || this.pendingCtrlPaste) {
          return;
        }
        this.pendingCtrlPaste = {
          code: e.code,
          heldCtrlCodes: new Set(this.activeControlCodes),
          releasedCtrlCodes: new Set<string>(),
        };
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
        const releasedCtrlCodes = this.releaseMacCtrlsForRemap();
        this.remappedKeys.set(e.code, { releasedCtrlCodes });
        this.sendKeyEvent(remapCode, '', true, false, false, e.shiftKey, false, false);
        this.sendKeyEvent(remapCode, '', false, false, false, e.shiftKey, false, false);
        this.restoreMacCtrls(releasedCtrlCodes);
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
        this.pendingComposition = {
          code: e.code,
          shift: e.shiftKey,
          fallbackKey: e.key,
        };
        return;
      }

      if (this.pendingComposition?.code === e.code) {
        e.preventDefault();
        return;
      }

      e.preventDefault();

      if (this.deadKeyPending) {
        this.deadKeyPending = false;
        // Keep deadKeyCode set — we need it to suppress the dead key's own keyup
      }

      if (shouldMaterializeMacCtrl(e, {
        macMetaAsCtrl: this.macMetaAsCtrl,
        activeMacMetaCount: this.activeMacMetaCodes.size,
      })) {
        this.materializeMacCtrl();
      }
      if (shouldMaterializeMacOption(e, {
        isMac: this.isMac,
        activeMacOptionCount: this.activeMacOptionCodes.size,
        macOptionComposition: isMacOptionComposition(e, {
          isMac: this.isMac,
          activeMacOptionCount: this.activeMacOptionCodes.size,
        }),
      })) {
        this.materializeMacOption();
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
      if (isMacOptionComposition(e, {
        isMac: this.isMac,
        activeMacOptionCount: this.activeMacOptionCodes.size,
      })) {
        altgr = true;
        alt = false;
      }

      this.sendKeyEvent(e.code, e.key, true, ctrl, alt, e.shiftKey, meta && !this.macMetaAsCtrl, altgr);
    }, { signal });

    keyboardTarget.addEventListener('keyup', (e: KeyboardEvent) => {
      // Clear remapped keys (e.g., Cmd+Left was sent as atomic Home)
      const remapped = this.remappedKeys.get(e.code);
      if (remapped) {
        e.preventDefault();
        this.remappedKeys.delete(e.code);
        return;
      }

      if (isMacMetaKey(e.code, this.macMetaAsCtrl)) {
        e.preventDefault();
        this.activeMacMetaCodes.delete(e.code);
        const ctrlCode = MAC_META_TO_CTRL[e.code];
        if (ctrlCode && this.materializedMacCtrlCodes.has(ctrlCode)) {
          this.materializedMacCtrlCodes.delete(ctrlCode);
          this.sendKeyEvent(ctrlCode, '', false, false, false, false, false, false);
        }
        return;
      }

      if (isMacOptionKey(e.code, this.isMac)) {
        e.preventDefault();
        this.activeMacOptionCodes.delete(e.code);
        if (this.materializedMacOptionCodes.has(e.code)) {
          this.materializedMacOptionCodes.delete(e.code);
          this.sendKeyEvent(e.code, '', false, false, false, false, false, false);
        }
        return;
      }

      if (CTRL_KEY_CODES.has(e.code)) {
        if (this.pendingCtrlPaste?.heldCtrlCodes.has(e.code)) {
          e.preventDefault();
          this.activeControlCodes.delete(e.code);
          this.pendingCtrlPaste.releasedCtrlCodes.add(e.code);
          return;
        }
        this.activeControlCodes.delete(e.code);
      }

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

      if (this.pendingComposition?.code === e.code) {
        e.preventDefault();
        this.schedulePendingCompositionFallback();
        return;
      }

      if (e.isComposing) return;

      // Suppress the dead key's own keyup (unpaired release prevention)
      if (e.code === this.deadKeyCode) {
        this.deadKeyCode = null;
        return;
      }

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
      if (isMacOptionComposition(e, {
        isMac: this.isMac,
        activeMacOptionCount: this.activeMacOptionCodes.size,
      })) {
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
    this.activeMacMetaCodes.clear();
    this.activeControlCodes.clear();
    this.materializedMacCtrlCodes.clear();
    this.activeMacOptionCodes.clear();
    this.materializedMacOptionCodes.clear();
    this.remappedKeys.clear();
    this.pendingComposition = null;
    this.pendingSyntheticAccent = null;
    this.pendingCtrlPaste = null;
    this.clearPendingCompositionFallback();
    this.suppressedKeyups.reset();
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
  ): void {
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
      return;
    }
    this.sendFrame(CH_INPUT, payload);
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

  private commitPendingComposition(text: string): void {
    if (!this.pendingComposition || text.length !== 1) return;
    const pending = this.pendingComposition;
    this.pendingComposition = null;
    this.clearPendingCompositionFallback();
    this.sendKeyEvent(pending.code, text, true, false, false, pending.shift, false, false);
    this.sendKeyEvent(pending.code, text, false, false, false, pending.shift, false, false);
    this.suppressedKeyups.suppress(pending.code);
    this.keyboardSink.clear();
  }

  private schedulePendingCompositionFallback(): void {
    if (!this.pendingComposition) return;
    this.clearPendingCompositionFallback();
    this.pendingCompositionFallbackTimer = window.setTimeout(() => {
      this.pendingCompositionFallbackTimer = null;
      const pending = this.pendingComposition;
      this.pendingComposition = null;
      if (pending?.fallbackKey.length === 1) {
        this.sendKeyEvent(pending.code, pending.fallbackKey, true, false, false, pending.shift, false, false);
        this.sendKeyEvent(pending.code, pending.fallbackKey, false, false, false, pending.shift, false, false);
      }
      this.keyboardSink.clear();
    }, PENDING_COMPOSITION_FALLBACK_MS);
  }

  private clearPendingCompositionFallback(): void {
    if (this.pendingCompositionFallbackTimer !== null) {
      window.clearTimeout(this.pendingCompositionFallbackTimer);
      this.pendingCompositionFallbackTimer = null;
    }
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

  private materializeMacCtrl(): void {
    for (const metaCode of this.activeMacMetaCodes) {
      const ctrlCode = MAC_META_TO_CTRL[metaCode];
      if (!ctrlCode || this.materializedMacCtrlCodes.has(ctrlCode)) continue;
      this.sendKeyEvent(ctrlCode, '', true, false, false, false, false, false);
      this.materializedMacCtrlCodes.add(ctrlCode);
    }
  }

  private materializeMacOption(): void {
    for (const optionCode of this.activeMacOptionCodes) {
      if (this.materializedMacOptionCodes.has(optionCode)) continue;
      this.sendKeyEvent(optionCode, '', true, false, false, false, false, false);
      this.materializedMacOptionCodes.add(optionCode);
    }
  }

  private releaseMacCtrlsForRemap(): string[] {
    const released: string[] = [];
    for (const metaCode of this.activeMacMetaCodes) {
      const ctrlCode = MAC_META_TO_CTRL[metaCode];
      if (!ctrlCode || !this.materializedMacCtrlCodes.has(ctrlCode)) continue;
      this.materializedMacCtrlCodes.delete(ctrlCode);
      this.sendKeyEvent(ctrlCode, '', false, false, false, false, false, false);
      released.push(ctrlCode);
    }
    return released;
  }

  private restoreMacCtrls(ctrlCodes: Iterable<string>): void {
    for (const ctrlCode of ctrlCodes) {
      const metaCode = Object.entries(MAC_META_TO_CTRL).find(([, mappedCtrl]) => mappedCtrl === ctrlCode)?.[0];
      if (!metaCode || !this.activeMacMetaCodes.has(metaCode) || this.materializedMacCtrlCodes.has(ctrlCode)) {
        continue;
      }
      this.sendKeyEvent(ctrlCode, '', true, false, false, false, false, false);
      this.materializedMacCtrlCodes.add(ctrlCode);
    }
  }

  private dispatchAtomicMacCtrlShortcut(code: string, key: string): void {
    const ctrlCode = this.preferredMacCtrlCode();
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
    const pending = this.pendingCtrlPaste;
    if (!pending) return;

    this.pendingCtrlPaste = null;
    this.sendKeyEvent(pending.code, '', true, true, false, false, false, false);
    this.sendKeyEvent(pending.code, '', false, true, false, false, false, false);

    for (const ctrlCode of pending.releasedCtrlCodes) {
      this.sendKeyEvent(ctrlCode, '', false, false, false, false, false, false);
    }
  }

  private preferredMacCtrlCode(): string {
    for (const metaCode of this.activeMacMetaCodes) {
      const ctrlCode = MAC_META_TO_CTRL[metaCode];
      if (ctrlCode) {
        return ctrlCode;
      }
    }
    return 'ControlLeft';
  }
}

export { inferLayoutName, inferLayoutHint } from './input/layout-hint.js';
