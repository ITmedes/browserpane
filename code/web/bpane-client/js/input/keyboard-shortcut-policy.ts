const MAC_META_ATOMIC_SHORTCUTS = new Set(['KeyC', 'KeyV']);
const MAC_META_TO_CTRL: Record<string, string> = {
  MetaLeft: 'ControlLeft',
  MetaRight: 'ControlRight',
};
const MAC_OPTION_CODES = new Set(['AltLeft', 'AltRight']);
const KEYBOARD_MODIFIER_CODES = new Set([
  'ShiftLeft', 'ShiftRight',
  'ControlLeft', 'ControlRight',
  'AltLeft', 'AltRight',
  'MetaLeft', 'MetaRight',
]);

type KeyEventLike = {
  code: string;
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
};

export function isMacMetaKey(code: string, macMetaAsCtrl: boolean): boolean {
  return macMetaAsCtrl && Object.hasOwn(MAC_META_TO_CTRL, code);
}

export function isMacOptionKey(code: string, isMac: boolean): boolean {
  return isMac && MAC_OPTION_CODES.has(code);
}

export function isMacOptionComposition(
  event: KeyEventLike,
  input: {
    isMac: boolean;
    activeMacOptionCount: number;
  },
): boolean {
  return input.isMac
    && input.activeMacOptionCount > 0
    && !event.ctrlKey
    && !event.metaKey
    && (event.key === 'Dead' || event.key.length === 1);
}

export function shouldMaterializeMacCtrl(
  event: KeyEventLike,
  input: {
    macMetaAsCtrl: boolean;
    activeMacMetaCount: number;
  },
): boolean {
  return input.macMetaAsCtrl
    && event.metaKey
    && !KEYBOARD_MODIFIER_CODES.has(event.code)
    && input.activeMacMetaCount > 0;
}

export function shouldSuppressLockedWindowShortcut(
  event: KeyEventLike,
  input: {
    isMac: boolean;
  },
): boolean {
  if (!event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey && event.code === 'F11') {
    return true;
  }

  if (!event.ctrlKey && event.altKey && !event.metaKey && !event.shiftKey && event.code === 'F4') {
    return true;
  }

  return !input.isMac
    && event.ctrlKey
    && !event.altKey
    && !event.metaKey
    && (event.code === 'KeyQ' || event.code === 'KeyW');
}

export function shouldSendAtomicMacCtrlShortcut(
  event: KeyEventLike,
  input: {
    macMetaAsCtrl: boolean;
  },
): boolean {
  return input.macMetaAsCtrl
    && event.metaKey
    && !event.ctrlKey
    && !event.altKey
    && !event.shiftKey
    && MAC_META_ATOMIC_SHORTCUTS.has(event.code);
}

export function shouldDeferCtrlPasteShortcut(
  event: KeyEventLike,
  input: {
    clipboardEnabled: boolean;
    activeControlCount: number;
  },
): boolean {
  return input.clipboardEnabled
    && event.code === 'KeyV'
    && event.ctrlKey
    && !event.altKey
    && !event.metaKey
    && !event.shiftKey
    && input.activeControlCount > 0;
}

export function shouldMaterializeMacOption(
  event: KeyEventLike,
  input: {
    isMac: boolean;
    activeMacOptionCount: number;
    macOptionComposition: boolean;
  },
): boolean {
  return input.isMac
    && event.altKey
    && !event.ctrlKey
    && !event.metaKey
    && !KEYBOARD_MODIFIER_CODES.has(event.code)
    && input.activeMacOptionCount > 0
    && !input.macOptionComposition;
}
