import type { AtomicMacShortcutDispatcher } from './atomic-mac-shortcut-dispatcher.js';
import type { ClipboardSyncRuntime } from './clipboard-sync-runtime.js';
import type { DeadKeyStateRuntime } from './dead-key-state-runtime.js';
import type { DeferredCtrlPasteRuntime } from './deferred-ctrl-paste-runtime.js';
import type { KeyEventStateResolver } from './key-event-state-resolver.js';
import type { KeyboardSinkRuntime } from './keyboard-sink-runtime.js';
import type { MacModifierStateRuntime } from './mac-modifier-state-runtime.js';
import type { MacNavigationRemapRuntime } from './mac-navigation-remap-runtime.js';
import type { MacNavigationShortcutDispatcher } from './mac-navigation-shortcut-dispatcher.js';
import type { PendingCompositionRuntime } from './pending-composition-runtime.js';
import type { ShortcutGatingPolicy } from './shortcut-gating-policy.js';
import type { ShortcutKeyReleaseDispatcher } from './shortcut-key-release-dispatcher.js';
import type {
  ShortcutKeyReleaseRuntime,
  ShortcutReleasedKey,
} from './shortcut-key-release-runtime.js';
import type { SuppressedKeyupTracker } from './suppressed-keyup-tracker.js';
import type { SupportedDeadAccent } from './synthetic-dead-accent.js';
import type { SyntheticDeadAccentRuntime } from './synthetic-dead-accent-runtime.js';

const CTRL_KEY_CODES = new Set(['ControlLeft', 'ControlRight']);

export interface KeyboardDispatchEvent extends ShortcutReleasedKey {
  down: boolean;
}

export interface KeyboardInputRuntimeInput {
  clipboardEnabled: boolean;
  isMac: boolean;
  macMetaAsCtrl: boolean;
  keyboardSink: Pick<KeyboardSinkRuntime, 'clear'>;
  suppressedKeyups: Pick<SuppressedKeyupTracker, 'clear' | 'suppress'>;
  macModifiers: Pick<
    MacModifierStateRuntime,
    | 'handleMetaKeyup'
    | 'handleOptionKeyup'
    | 'isMacMetaKey'
    | 'isMacOptionComposition'
    | 'isMacOptionKey'
    | 'materializeMacCtrl'
    | 'materializeMacOption'
    | 'noteMetaKeydown'
    | 'noteOptionKeydown'
    | 'releaseMacCtrlsForRemap'
    | 'shouldMaterializeMacCtrl'
    | 'shouldMaterializeMacOption'
  >;
  deferredCtrlPaste: Pick<
    DeferredCtrlPasteRuntime,
    | 'begin'
    | 'flush'
    | 'noteControlKeydown'
    | 'noteControlKeyup'
    | 'shouldDeferPaste'
  >;
  macNavigationRemap: Pick<
    MacNavigationRemapRuntime,
    'handleKeyup' | 'hasActiveRemap'
  >;
  resolveSupportedDeadAccent: (event: KeyboardEvent, isMac: boolean) => SupportedDeadAccent | null;
  deadKeyState: Pick<
    DeadKeyStateRuntime,
    | 'applySyntheticAccentFallback'
    | 'beginPendingCompositionIfNeeded'
    | 'clearTrackedDeadKey'
    | 'consumeTrackedDeadKeyKeyup'
    | 'noteNativeDeadKey'
    | 'shouldIgnoreComposingKeydown'
    | 'startSupportedDeadAccent'
  >;
  syntheticDeadAccent: Pick<
    SyntheticDeadAccentRuntime,
    'begin' | 'handleKeydown' | 'handleKeyup'
  >;
  shortcutPolicy: Pick<
    ShortcutGatingPolicy,
    | 'shouldPassThroughMacMetaShortcut'
    | 'shouldSendAtomicMacCtrlShortcut'
    | 'shouldSuppressLockedWindowShortcut'
  >;
  atomicMacShortcutDispatcher: Pick<
    AtomicMacShortcutDispatcher,
    'dispatchShortcutWithClipboardSync'
  >;
  clipboardSync: Pick<
    ClipboardSyncRuntime,
    'refreshClipboardText' | 'syncClipboardBeforePaste'
  >;
  macNavigationShortcutDispatcher: Pick<
    MacNavigationShortcutDispatcher,
    'dispatchShortcut'
  >;
  pendingComposition: Pick<
    PendingCompositionRuntime,
    'handleKeyup' | 'hasPendingCode'
  >;
  keyEventStateResolver: Pick<KeyEventStateResolver, 'resolve'>;
  shortcutKeyRelease: Pick<
    ShortcutKeyReleaseRuntime,
    'noteObservedKeyup' | 'noteSentKeydown'
  >;
  shortcutKeyReleaseDispatcher: Pick<
    ShortcutKeyReleaseDispatcher,
    'releaseKeysForModifierKeyup'
  >;
  emitKeyEvent: (event: KeyboardDispatchEvent) => boolean;
}

export interface KeyboardInputRuntimeBindInput {
  keyboardTarget: HTMLElement;
  signal: AbortSignal;
}

export class KeyboardInputRuntime {
  private readonly clipboardEnabled: boolean;
  private readonly isMac: boolean;
  private readonly macMetaAsCtrl: boolean;
  private readonly keyboardSink: Pick<KeyboardSinkRuntime, 'clear'>;
  private readonly suppressedKeyups: Pick<SuppressedKeyupTracker, 'clear' | 'suppress'>;
  private readonly macModifiers: KeyboardInputRuntimeInput['macModifiers'];
  private readonly deferredCtrlPaste: KeyboardInputRuntimeInput['deferredCtrlPaste'];
  private readonly macNavigationRemap: KeyboardInputRuntimeInput['macNavigationRemap'];
  private readonly resolveSupportedDeadAccent: KeyboardInputRuntimeInput['resolveSupportedDeadAccent'];
  private readonly deadKeyState: KeyboardInputRuntimeInput['deadKeyState'];
  private readonly syntheticDeadAccent: KeyboardInputRuntimeInput['syntheticDeadAccent'];
  private readonly shortcutPolicy: KeyboardInputRuntimeInput['shortcutPolicy'];
  private readonly atomicMacShortcutDispatcher: KeyboardInputRuntimeInput['atomicMacShortcutDispatcher'];
  private readonly clipboardSync: KeyboardInputRuntimeInput['clipboardSync'];
  private readonly macNavigationShortcutDispatcher: KeyboardInputRuntimeInput['macNavigationShortcutDispatcher'];
  private readonly pendingComposition: KeyboardInputRuntimeInput['pendingComposition'];
  private readonly keyEventStateResolver: KeyboardInputRuntimeInput['keyEventStateResolver'];
  private readonly shortcutKeyRelease: KeyboardInputRuntimeInput['shortcutKeyRelease'];
  private readonly shortcutKeyReleaseDispatcher: KeyboardInputRuntimeInput['shortcutKeyReleaseDispatcher'];
  private readonly emitKeyEvent: (event: KeyboardDispatchEvent) => boolean;

  constructor(input: KeyboardInputRuntimeInput) {
    this.clipboardEnabled = input.clipboardEnabled;
    this.isMac = input.isMac;
    this.macMetaAsCtrl = input.macMetaAsCtrl;
    this.keyboardSink = input.keyboardSink;
    this.suppressedKeyups = input.suppressedKeyups;
    this.macModifiers = input.macModifiers;
    this.deferredCtrlPaste = input.deferredCtrlPaste;
    this.macNavigationRemap = input.macNavigationRemap;
    this.resolveSupportedDeadAccent = input.resolveSupportedDeadAccent;
    this.deadKeyState = input.deadKeyState;
    this.syntheticDeadAccent = input.syntheticDeadAccent;
    this.shortcutPolicy = input.shortcutPolicy;
    this.atomicMacShortcutDispatcher = input.atomicMacShortcutDispatcher;
    this.clipboardSync = input.clipboardSync;
    this.macNavigationShortcutDispatcher = input.macNavigationShortcutDispatcher;
    this.pendingComposition = input.pendingComposition;
    this.keyEventStateResolver = input.keyEventStateResolver;
    this.shortcutKeyRelease = input.shortcutKeyRelease;
    this.shortcutKeyReleaseDispatcher = input.shortcutKeyReleaseDispatcher;
    this.emitKeyEvent = input.emitKeyEvent;
  }

  bind(input: KeyboardInputRuntimeBindInput): void {
    input.keyboardTarget.addEventListener('keydown', (event: KeyboardEvent) => {
      this.handleKeydown(event);
    }, { signal: input.signal });

    input.keyboardTarget.addEventListener('keyup', (event: KeyboardEvent) => {
      this.handleKeyup(event);
    }, { signal: input.signal });
  }

  private handleKeydown(event: KeyboardEvent): void {
    if (!event.repeat) {
      this.suppressedKeyups.clear(event.code);
    }

    if (this.macModifiers.isMacMetaKey(event.code)) {
      event.preventDefault();
      this.macModifiers.noteMetaKeydown(event.code);
      return;
    }

    if (this.macModifiers.isMacOptionKey(event.code)) {
      event.preventDefault();
      this.macModifiers.noteOptionKeydown(event.code);
      return;
    }

    if (CTRL_KEY_CODES.has(event.code)) {
      this.deferredCtrlPaste.noteControlKeydown(event.code);
    }

    if (this.macNavigationRemap.hasActiveRemap(event.code)) {
      event.preventDefault();
      return;
    }

    const supportedDeadAccent = this.resolveSupportedDeadAccent(event, this.isMac);
    if (supportedDeadAccent) {
      event.preventDefault();
      this.deadKeyState.startSupportedDeadAccent(event.code);
      this.syntheticDeadAccent.begin(supportedDeadAccent, event.code);
      return;
    }

    const syntheticDeadAccentKeydown = this.syntheticDeadAccent.handleKeydown(event);
    if (syntheticDeadAccentKeydown.handled) {
      event.preventDefault();
      if (syntheticDeadAccentKeydown.clearKeyboardSink) {
        this.keyboardSink.clear();
      }
      return;
    }

    if (syntheticDeadAccentKeydown.fallback) {
      this.deadKeyState.applySyntheticAccentFallback(syntheticDeadAccentKeydown.fallback);
    }

    if (this.deadKeyState.shouldIgnoreComposingKeydown(event.isComposing)) {
      return;
    }

    if (this.shortcutPolicy.shouldSuppressLockedWindowShortcut(event)) {
      event.preventDefault();
      if (!event.repeat) {
        this.suppressedKeyups.suppress(event.code);
      }
      return;
    }

    if (this.shortcutPolicy.shouldPassThroughMacMetaShortcut(event)) {
      this.macModifiers.releaseMacCtrlsForRemap();
      return;
    }

    const effectiveCtrl = event.ctrlKey || (this.macMetaAsCtrl && event.metaKey);
    if (this.shortcutPolicy.shouldSendAtomicMacCtrlShortcut(event)) {
      event.preventDefault();
      if (event.repeat) {
        return;
      }
      this.suppressedKeyups.suppress(event.code);
      this.atomicMacShortcutDispatcher.dispatchShortcutWithClipboardSync({
        code: event.code,
        key: event.key,
        clipboardEnabled: this.clipboardEnabled,
      });
      return;
    }

    if (this.deferredCtrlPaste.shouldDeferPaste(event, this.clipboardEnabled)) {
      event.preventDefault();
      if (event.repeat || !this.deferredCtrlPaste.begin(event.code)) {
        return;
      }
      this.suppressedKeyups.suppress(event.code);
      void this.clipboardSync.syncClipboardBeforePaste().finally(() => {
        this.deferredCtrlPaste.flush();
      });
      return;
    }

    if (effectiveCtrl && event.code === 'KeyV' && !event.repeat && this.clipboardEnabled) {
      void this.clipboardSync.refreshClipboardText();
    }

    if (this.macMetaAsCtrl && event.metaKey && (event.code === 'ArrowLeft' || event.code === 'ArrowRight')) {
      event.preventDefault();
      this.macNavigationShortcutDispatcher.dispatchShortcut(event.code, event.shiftKey);
      return;
    }

    if (event.key === 'Dead') {
      this.deadKeyState.noteNativeDeadKey(event.code);
      return;
    }

    if (this.deadKeyState.beginPendingCompositionIfNeeded({
      code: event.code,
      shift: event.shiftKey,
      fallbackKey: event.key,
    })) {
      return;
    }

    if (this.pendingComposition.hasPendingCode(event.code)) {
      event.preventDefault();
      return;
    }

    event.preventDefault();

    if (this.macModifiers.shouldMaterializeMacCtrl(event)) {
      this.macModifiers.materializeMacCtrl();
    }
    if (this.macModifiers.shouldMaterializeMacOption(event)) {
      this.macModifiers.materializeMacOption();
    }

    const resolvedState = this.keyEventStateResolver.resolve(event);
    if (this.emitKeyEvent({
      code: event.code,
      key: event.key,
      down: true,
      ctrl: resolvedState.ctrl,
      alt: resolvedState.alt,
      shift: resolvedState.shift,
      meta: resolvedState.meta,
      altgr: resolvedState.altgr,
    })) {
      this.shortcutKeyRelease.noteSentKeydown({
        code: event.code,
        key: event.key,
        ctrl: resolvedState.ctrl,
        alt: resolvedState.alt,
        shift: resolvedState.shift,
        meta: resolvedState.meta,
        altgr: resolvedState.altgr,
      });
    }
  }

  private handleKeyup(event: KeyboardEvent): void {
    const remapped = this.macNavigationRemap.handleKeyup(event.code);
    if (remapped) {
      event.preventDefault();
      return;
    }

    if (this.macModifiers.isMacMetaKey(event.code)) {
      this.shortcutKeyReleaseDispatcher.releaseKeysForModifierKeyup(event);
      this.macModifiers.handleMetaKeyup(event.code);
      event.preventDefault();
      return;
    }

    if (this.macModifiers.isMacOptionKey(event.code)) {
      this.shortcutKeyReleaseDispatcher.releaseKeysForModifierKeyup(event);
      this.macModifiers.handleOptionKeyup(event.code);
      event.preventDefault();
      return;
    }

    if (CTRL_KEY_CODES.has(event.code)) {
      if (this.deferredCtrlPaste.noteControlKeyup(event.code)) {
        event.preventDefault();
        return;
      }
    }

    this.shortcutKeyRelease.noteObservedKeyup(event.code);

    const syntheticDeadAccentKeyup = this.syntheticDeadAccent.handleKeyup(event.code);
    if (syntheticDeadAccentKeyup.handled) {
      event.preventDefault();
      if (syntheticDeadAccentKeyup.emitCharacter) {
        this.emitKeyEvent({
          code: event.code,
          key: syntheticDeadAccentKeyup.emitCharacter.key,
          down: true,
          ctrl: false,
          alt: false,
          shift: false,
          meta: false,
          altgr: false,
        });
        this.emitKeyEvent({
          code: event.code,
          key: syntheticDeadAccentKeyup.emitCharacter.key,
          down: false,
          ctrl: false,
          alt: false,
          shift: false,
          meta: false,
          altgr: false,
        });
      }
      if (syntheticDeadAccentKeyup.clearDeadKeyCode) {
        this.deadKeyState.clearTrackedDeadKey();
      }
      return;
    }

    if (this.suppressedKeyups.clear(event.code)) {
      event.preventDefault();
      return;
    }

    if (this.pendingComposition.handleKeyup(event.code)) {
      event.preventDefault();
      return;
    }

    if (event.isComposing) {
      return;
    }

    if (this.deadKeyState.consumeTrackedDeadKeyKeyup(event.code)) {
      return;
    }

    this.shortcutKeyReleaseDispatcher.releaseKeysForModifierKeyup(event);
    event.preventDefault();

    const resolvedState = this.keyEventStateResolver.resolve(event);
    this.emitKeyEvent({
      code: event.code,
      key: event.key,
      down: false,
      ctrl: resolvedState.ctrl,
      alt: resolvedState.alt,
      shift: resolvedState.shift,
      meta: resolvedState.meta,
      altgr: resolvedState.altgr,
    });
  }
}
