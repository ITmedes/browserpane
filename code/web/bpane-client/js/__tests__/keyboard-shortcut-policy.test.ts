import { describe, expect, it } from 'vitest';
import {
  isMacMetaKey,
  isMacOptionComposition,
  isMacOptionKey,
  shouldDeferCtrlPasteShortcut,
  shouldMaterializeMacCtrl,
  shouldMaterializeMacOption,
  shouldSendAtomicMacCtrlShortcut,
  shouldSuppressLockedWindowShortcut,
} from '../input/keyboard-shortcut-policy.js';

function createKeyEventLike(input: {
  code: string;
  key?: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}) {
  return {
    code: input.code,
    key: input.key ?? '',
    ctrlKey: input.ctrlKey ?? false,
    altKey: input.altKey ?? false,
    metaKey: input.metaKey ?? false,
    shiftKey: input.shiftKey ?? false,
  };
}

describe('keyboard shortcut policy', () => {
  it('detects mac meta and option modifier keys', () => {
    expect(isMacMetaKey('MetaLeft', true)).toBe(true);
    expect(isMacMetaKey('MetaRight', true)).toBe(true);
    expect(isMacMetaKey('MetaLeft', false)).toBe(false);
    expect(isMacMetaKey('KeyA', true)).toBe(false);

    expect(isMacOptionKey('AltLeft', true)).toBe(true);
    expect(isMacOptionKey('AltRight', true)).toBe(true);
    expect(isMacOptionKey('AltLeft', false)).toBe(false);
    expect(isMacOptionKey('KeyA', true)).toBe(false);
  });

  it('detects mac option composition only for printable/dead keys with an active option key', () => {
    expect(isMacOptionComposition(
      createKeyEventLike({ code: 'KeyL', key: '@' }),
      { isMac: true, activeMacOptionCount: 1 },
    )).toBe(true);
    expect(isMacOptionComposition(
      createKeyEventLike({ code: 'KeyE', key: 'Dead' }),
      { isMac: true, activeMacOptionCount: 1 },
    )).toBe(true);
    expect(isMacOptionComposition(
      createKeyEventLike({ code: 'KeyL', key: '@', ctrlKey: true }),
      { isMac: true, activeMacOptionCount: 1 },
    )).toBe(false);
    expect(isMacOptionComposition(
      createKeyEventLike({ code: 'ArrowLeft', key: 'ArrowLeft', altKey: true }),
      { isMac: true, activeMacOptionCount: 1 },
    )).toBe(false);
    expect(isMacOptionComposition(
      createKeyEventLike({ code: 'KeyL', key: '@' }),
      { isMac: false, activeMacOptionCount: 1 },
    )).toBe(false);
  });

  it('detects locked-window shortcuts across platforms', () => {
    expect(shouldSuppressLockedWindowShortcut(
      createKeyEventLike({ code: 'F11' }),
      { isMac: false },
    )).toBe(true);
    expect(shouldSuppressLockedWindowShortcut(
      createKeyEventLike({ code: 'F4', altKey: true }),
      { isMac: false },
    )).toBe(true);
    expect(shouldSuppressLockedWindowShortcut(
      createKeyEventLike({ code: 'KeyW', ctrlKey: true }),
      { isMac: false },
    )).toBe(true);
    expect(shouldSuppressLockedWindowShortcut(
      createKeyEventLike({ code: 'KeyW', ctrlKey: true }),
      { isMac: true },
    )).toBe(false);
  });

  it('detects atomic mac command shortcuts and deferred ctrl paste', () => {
    expect(shouldSendAtomicMacCtrlShortcut(
      createKeyEventLike({ code: 'KeyC', metaKey: true }),
      { macMetaAsCtrl: true },
    )).toBe(true);
    expect(shouldSendAtomicMacCtrlShortcut(
      createKeyEventLike({ code: 'KeyV', metaKey: true }),
      { macMetaAsCtrl: true },
    )).toBe(true);
    expect(shouldSendAtomicMacCtrlShortcut(
      createKeyEventLike({ code: 'KeyV', metaKey: true, shiftKey: true }),
      { macMetaAsCtrl: true },
    )).toBe(false);
    expect(shouldSendAtomicMacCtrlShortcut(
      createKeyEventLike({ code: 'KeyV', metaKey: true }),
      { macMetaAsCtrl: false },
    )).toBe(false);

    expect(shouldDeferCtrlPasteShortcut(
      createKeyEventLike({ code: 'KeyV', ctrlKey: true }),
      { clipboardEnabled: true, activeControlCount: 1 },
    )).toBe(true);
    expect(shouldDeferCtrlPasteShortcut(
      createKeyEventLike({ code: 'KeyV', ctrlKey: true, shiftKey: true }),
      { clipboardEnabled: true, activeControlCount: 1 },
    )).toBe(false);
    expect(shouldDeferCtrlPasteShortcut(
      createKeyEventLike({ code: 'KeyV', ctrlKey: true }),
      { clipboardEnabled: false, activeControlCount: 1 },
    )).toBe(false);
  });

  it('detects materialization rules for mac meta and option modifiers', () => {
    expect(shouldMaterializeMacCtrl(
      createKeyEventLike({ code: 'KeyA', metaKey: true }),
      { macMetaAsCtrl: true, activeMacMetaCount: 1 },
    )).toBe(true);
    expect(shouldMaterializeMacCtrl(
      createKeyEventLike({ code: 'MetaLeft', metaKey: true }),
      { macMetaAsCtrl: true, activeMacMetaCount: 1 },
    )).toBe(false);
    expect(shouldMaterializeMacCtrl(
      createKeyEventLike({ code: 'KeyA', metaKey: true }),
      { macMetaAsCtrl: false, activeMacMetaCount: 1 },
    )).toBe(false);

    expect(shouldMaterializeMacOption(
      createKeyEventLike({ code: 'ArrowLeft', key: 'ArrowLeft', altKey: true }),
      { isMac: true, activeMacOptionCount: 1, macOptionComposition: false },
    )).toBe(true);
    expect(shouldMaterializeMacOption(
      createKeyEventLike({ code: 'KeyL', key: '@', altKey: true }),
      { isMac: true, activeMacOptionCount: 1, macOptionComposition: true },
    )).toBe(false);
    expect(shouldMaterializeMacOption(
      createKeyEventLike({ code: 'AltLeft', key: 'Alt', altKey: true }),
      { isMac: true, activeMacOptionCount: 1, macOptionComposition: false },
    )).toBe(false);
    expect(shouldMaterializeMacOption(
      createKeyEventLike({ code: 'ArrowLeft', key: 'ArrowLeft', altKey: true }),
      { isMac: false, activeMacOptionCount: 1, macOptionComposition: false },
    )).toBe(false);
  });
});
