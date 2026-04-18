import { describe, expect, it } from 'vitest';
import { ShortcutGatingPolicy } from '../input/shortcut-gating-policy.js';

function createEvent(input: {
  code: string;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}) {
  return {
    code: input.code,
    ctrlKey: input.ctrlKey ?? false,
    altKey: input.altKey ?? false,
    metaKey: input.metaKey ?? false,
    shiftKey: input.shiftKey ?? false,
  };
}

describe('ShortcutGatingPolicy', () => {
  it('suppresses locked-window shortcuts for the supported platforms', () => {
    const linuxPolicy = new ShortcutGatingPolicy({
      isMac: false,
      macMetaAsCtrl: false,
    });
    const macPolicy = new ShortcutGatingPolicy({
      isMac: true,
      macMetaAsCtrl: true,
    });

    expect(linuxPolicy.shouldSuppressLockedWindowShortcut(createEvent({ code: 'F11' }))).toBe(true);
    expect(linuxPolicy.shouldSuppressLockedWindowShortcut(createEvent({ code: 'F4', altKey: true }))).toBe(true);
    expect(linuxPolicy.shouldSuppressLockedWindowShortcut(createEvent({ code: 'KeyW', ctrlKey: true }))).toBe(true);
    expect(macPolicy.shouldSuppressLockedWindowShortcut(createEvent({ code: 'KeyW', ctrlKey: true }))).toBe(false);
  });

  it('passes through only the allowed mac meta shortcuts', () => {
    const policy = new ShortcutGatingPolicy({
      isMac: true,
      macMetaAsCtrl: true,
    });

    expect(policy.shouldPassThroughMacMetaShortcut(createEvent({ code: 'KeyQ', metaKey: true }))).toBe(true);
    expect(policy.shouldPassThroughMacMetaShortcut(createEvent({ code: 'Tab', metaKey: true }))).toBe(true);
    expect(policy.shouldPassThroughMacMetaShortcut(createEvent({ code: 'KeyC', metaKey: true }))).toBe(false);
    expect(policy.shouldPassThroughMacMetaShortcut(createEvent({ code: 'KeyQ', metaKey: false }))).toBe(false);
  });

  it('detects atomic mac command shortcuts only for bare Cmd+C and Cmd+V', () => {
    const policy = new ShortcutGatingPolicy({
      isMac: true,
      macMetaAsCtrl: true,
    });
    const disabledPolicy = new ShortcutGatingPolicy({
      isMac: true,
      macMetaAsCtrl: false,
    });

    expect(policy.shouldSendAtomicMacCtrlShortcut(createEvent({ code: 'KeyC', metaKey: true }))).toBe(true);
    expect(policy.shouldSendAtomicMacCtrlShortcut(createEvent({ code: 'KeyV', metaKey: true }))).toBe(true);
    expect(policy.shouldSendAtomicMacCtrlShortcut(createEvent({ code: 'KeyV', metaKey: true, shiftKey: true }))).toBe(false);
    expect(policy.shouldSendAtomicMacCtrlShortcut(createEvent({ code: 'KeyV', metaKey: true, altKey: true }))).toBe(false);
    expect(disabledPolicy.shouldSendAtomicMacCtrlShortcut(createEvent({ code: 'KeyV', metaKey: true }))).toBe(false);
  });
});
