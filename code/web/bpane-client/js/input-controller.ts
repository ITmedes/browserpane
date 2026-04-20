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
import { AtomicMacShortcutDispatcher } from './input/atomic-mac-shortcut-dispatcher.js';
import { ClipboardSyncRuntime } from './input/clipboard-sync-runtime.js';
import { CompositionTextRuntime } from './input/composition-text-runtime.js';
import { DeadKeyStateRuntime } from './input/dead-key-state-runtime.js';
import { DeferredCtrlPasteRuntime } from './input/deferred-ctrl-paste-runtime.js';
import { KeyEventStateResolver } from './input/key-event-state-resolver.js';
import { KeyboardInputRuntime } from './input/keyboard-input-runtime.js';
import { MacNavigationRemapRuntime } from './input/mac-navigation-remap-runtime.js';
import { MacNavigationShortcutDispatcher } from './input/mac-navigation-shortcut-dispatcher.js';
import { KeyboardSinkRuntime } from './input/keyboard-sink-runtime.js';
import {
  inferLayoutHint,
  inferLayoutName,
  sendKeyboardLayoutHint,
} from './input/layout-hint.js';
import { PointerInputRuntime } from './input/pointer-input-runtime.js';
import { ShortcutGatingPolicy } from './input/shortcut-gating-policy.js';
import { ShortcutKeyReleaseDispatcher } from './input/shortcut-key-release-dispatcher.js';
import { ShortcutKeyReleaseRuntime } from './input/shortcut-key-release-runtime.js';
import { SuppressedKeyupTracker } from './input/suppressed-keyup-tracker.js';
import { SyntheticDeadAccentRuntime } from './input/synthetic-dead-accent-runtime.js';
import {
  resolveSupportedDeadAccent,
} from './input/synthetic-dead-accent.js';
import { MacModifierStateRuntime } from './input/mac-modifier-state-runtime.js';
import { PendingCompositionRuntime } from './input/pending-composition-runtime.js';

const PENDING_COMPOSITION_FALLBACK_MS = 16;
const SUPPRESSED_KEYUP_TIMEOUT_MS = 750;

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

  private inputAbortController: AbortController | null = null;
  private lastClipboardHash: bigint = 0n;
  private readonly clipboardSync: ClipboardSyncRuntime;
  private readonly compositionText: CompositionTextRuntime;
  private readonly deadKeyState: DeadKeyStateRuntime;
  private readonly keyboardSink: KeyboardSinkRuntime;
  private readonly pointerInput: PointerInputRuntime;
  private readonly suppressedKeyups: SuppressedKeyupTracker;
  private readonly pendingComposition: PendingCompositionRuntime;
  private readonly macModifiers: MacModifierStateRuntime;
  private readonly keyEventStateResolver: KeyEventStateResolver;
  private readonly macNavigationRemap: MacNavigationRemapRuntime;
  private readonly macNavigationShortcutDispatcher: MacNavigationShortcutDispatcher;
  private readonly deferredCtrlPaste: DeferredCtrlPasteRuntime;
  private readonly atomicMacShortcutDispatcher: AtomicMacShortcutDispatcher;
  private readonly shortcutPolicy: ShortcutGatingPolicy;
  private readonly shortcutKeyRelease: ShortcutKeyReleaseRuntime;
  private readonly shortcutKeyReleaseDispatcher: ShortcutKeyReleaseDispatcher;
  private readonly syntheticDeadAccent: SyntheticDeadAccentRuntime;
  private readonly keyboardInput: KeyboardInputRuntime;

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
    this.compositionText = new CompositionTextRuntime({
      commitText: (text) => {
        this.pendingComposition.commit(text);
      },
      getKeyboardSinkValue: () => this.keyboardSink.getValue(),
      clearKeyboardSink: () => {
        this.keyboardSink.clear();
      },
      documentLike: typeof document === 'undefined' ? undefined : document,
    });
    this.deadKeyState = new DeadKeyStateRuntime({
      resetPendingComposition: () => {
        this.pendingComposition.reset();
      },
      clearKeyboardSink: () => {
        this.keyboardSink.clear();
      },
      beginPendingComposition: (input) => {
        this.pendingComposition.begin(input);
      },
      emitSyntheticKeyEvent: (code, key, down) => {
        this.sendKeyEvent(code, key, down, false, false, false, false, false);
      },
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
    this.atomicMacShortcutDispatcher = new AtomicMacShortcutDispatcher({
      getPreferredCtrlCode: () => this.macModifiers.preferredMacCtrlCode(),
      emitKeyEvent: (event) => {
        this.sendKeyEvent(
          event.code,
          event.key,
          event.down,
          event.ctrl,
          event.alt,
          event.shift,
          event.meta,
          event.altgr,
        );
      },
      syncClipboardBeforePaste: () => this.clipboardSync.syncClipboardBeforePaste(),
    });
    this.shortcutPolicy = new ShortcutGatingPolicy({
      isMac: this.isMac,
      macMetaAsCtrl: this.macMetaAsCtrl,
    });
    this.shortcutKeyRelease = new ShortcutKeyReleaseRuntime();
    this.shortcutKeyReleaseDispatcher = new ShortcutKeyReleaseDispatcher({
      runtime: this.shortcutKeyRelease,
      emitKeyRelease: (release) => {
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
      },
      suppressKeyup: (code) => {
        this.suppressedKeyups.suppress(code);
      },
    });
    this.syntheticDeadAccent = new SyntheticDeadAccentRuntime();
    this.macModifiers = new MacModifierStateRuntime({
      isMac: this.isMac,
      macMetaAsCtrl: this.macMetaAsCtrl,
      emitModifierKey: (code, down) => {
        this.sendKeyEvent(code, '', down, false, false, false, false, false);
      },
    });
    this.keyEventStateResolver = new KeyEventStateResolver({
      macMetaAsCtrl: this.macMetaAsCtrl,
      isMacOptionComposition: (event) => this.macModifiers.isMacOptionComposition(event),
    });
    this.macNavigationRemap = new MacNavigationRemapRuntime();
    this.macNavigationShortcutDispatcher = new MacNavigationShortcutDispatcher({
      remapRuntime: this.macNavigationRemap,
      releaseMacCtrlsForRemap: () => this.macModifiers.releaseMacCtrlsForRemap(),
      restoreMacCtrls: (ctrlCodes) => this.macModifiers.restoreMacCtrls(ctrlCodes),
      emitNavigationKey: (code, down, shift) => {
        this.sendKeyEvent(code, '', down, false, false, shift, false, false);
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
    this.keyboardInput = new KeyboardInputRuntime({
      clipboardEnabled: this.clipboardEnabled,
      isMac: this.isMac,
      macMetaAsCtrl: this.macMetaAsCtrl,
      keyboardSink: this.keyboardSink,
      suppressedKeyups: this.suppressedKeyups,
      macModifiers: this.macModifiers,
      deferredCtrlPaste: this.deferredCtrlPaste,
      macNavigationRemap: this.macNavigationRemap,
      resolveSupportedDeadAccent,
      deadKeyState: this.deadKeyState,
      syntheticDeadAccent: this.syntheticDeadAccent,
      shortcutPolicy: this.shortcutPolicy,
      atomicMacShortcutDispatcher: this.atomicMacShortcutDispatcher,
      clipboardSync: this.clipboardSync,
      macNavigationShortcutDispatcher: this.macNavigationShortcutDispatcher,
      pendingComposition: this.pendingComposition,
      keyEventStateResolver: this.keyEventStateResolver,
      shortcutKeyRelease: this.shortcutKeyRelease,
      shortcutKeyReleaseDispatcher: this.shortcutKeyReleaseDispatcher,
      emitKeyEvent: (event) => this.sendKeyEvent(
        event.code,
        event.key,
        event.down,
        event.ctrl,
        event.alt,
        event.shift,
        event.meta,
        event.altgr,
      ),
    });
  }

  /** Set up all DOM event listeners on the canvas. */
  setup(): void {
    this.inputAbortController = new AbortController();
    const signal = this.inputAbortController.signal;
    const keyboardTarget = this.keyboardSink.ensure();
    this.pointerInput.bind({
      signal,
      focusKeyboardTarget: () => {
        keyboardTarget.focus();
      },
    });
    this.keyboardInput.bind({
      keyboardTarget,
      signal,
    });

    this.compositionText.bind({
      keyboardTarget,
      signal,
    });

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
    this.deadKeyState.reset();
    this.deferredCtrlPaste.reset();
    this.macNavigationRemap.reset();
    this.macModifiers.reset();
    this.pendingComposition.reset();
    this.syntheticDeadAccent.reset();
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
}

export { inferLayoutName, inferLayoutHint } from './input/layout-hint.js';
