/**
 * DOM KeyboardEvent.code → Linux evdev keycode mapping.
 */
const EVDEV_MAP: Record<string, number> = {
  Escape: 1, Digit1: 2, Digit2: 3, Digit3: 4, Digit4: 5,
  Digit5: 6, Digit6: 7, Digit7: 8, Digit8: 9, Digit9: 10,
  Digit0: 11, Minus: 12, Equal: 13, Backspace: 14, Tab: 15,
  KeyQ: 16, KeyW: 17, KeyE: 18, KeyR: 19, KeyT: 20,
  KeyY: 21, KeyU: 22, KeyI: 23, KeyO: 24, KeyP: 25,
  BracketLeft: 26, BracketRight: 27, Enter: 28, ControlLeft: 29,
  KeyA: 30, KeyS: 31, KeyD: 32, KeyF: 33, KeyG: 34,
  KeyH: 35, KeyJ: 36, KeyK: 37, KeyL: 38, Semicolon: 39,
  Quote: 40, Backquote: 41, ShiftLeft: 42, Backslash: 43,
  KeyZ: 44, KeyX: 45, KeyC: 46, KeyV: 47, KeyB: 48,
  KeyN: 49, KeyM: 50, Comma: 51, Period: 52, Slash: 53,
  ShiftRight: 54, NumpadMultiply: 55, AltLeft: 56, Space: 57, CapsLock: 58,
  F1: 59, F2: 60, F3: 61, F4: 62, F5: 63,
  F6: 64, F7: 65, F8: 66, F9: 67, F10: 68,
  NumLock: 69, ScrollLock: 70,
  Numpad7: 71, Numpad8: 72, Numpad9: 73, NumpadSubtract: 74,
  Numpad4: 75, Numpad5: 76, Numpad6: 77, NumpadAdd: 78,
  Numpad1: 79, Numpad2: 80, Numpad3: 81,
  Numpad0: 82, NumpadDecimal: 83,
  IntlBackslash: 86, // ISO key between LShift and Z (critical for DE: < > |)
  F11: 87, F12: 88,
  IntlRo: 89, // Japanese layout
  NumpadEnter: 96, ControlRight: 97, NumpadDivide: 98,
  PrintScreen: 99, AltRight: 100,
  Home: 102, ArrowUp: 103, PageUp: 104,
  ArrowLeft: 105, ArrowRight: 106,
  End: 107, ArrowDown: 108, PageDown: 109,
  Insert: 110, Delete: 111,
  Pause: 119,
  IntlYen: 124, // Japanese layout
  MetaLeft: 125, MetaRight: 126, ContextMenu: 127,
};

/**
 * Convert a DOM `KeyboardEvent.code` to a Linux evdev keycode.
 * Returns undefined for unmapped keys.
 */
export function domCodeToEvdev(code: string): number | undefined {
  return EVDEV_MAP[code];
}

/** Modifier bitmask constants (must match bpane-protocol). */
export const MOD_CTRL  = 0x01;
export const MOD_ALT   = 0x02;
export const MOD_SHIFT = 0x04;
export const MOD_META  = 0x08;
export const MOD_ALTGR = 0x10;

/**
 * Build a modifier bitmask from boolean flags.
 */
export function buildModifiers(ctrl: boolean, alt: boolean, shift: boolean, meta: boolean, altgr: boolean = false): number {
  let m = 0;
  if (ctrl)  m |= MOD_CTRL;
  if (alt)   m |= MOD_ALT;
  if (shift) m |= MOD_SHIFT;
  if (meta)  m |= MOD_META;
  if (altgr) m |= MOD_ALTGR;
  return m;
}

/**
 * Detect if running on macOS.
 */
export function isMacPlatform(): boolean {
  if (typeof navigator === 'undefined') return false;
  if ((navigator as any).userAgentData?.platform) {
    return (navigator as any).userAgentData.platform === 'macOS';
  }
  return navigator.platform?.startsWith('Mac') ?? false;
}

/**
 * Normalize a WheelEvent delta to discrete scroll steps.
 * Uses accumulator state so smooth/trackpad deltas still become wheel notches.
 */
export interface ScrollState {
  remainderX: number;
  remainderY: number;
}

export function createScrollState(): ScrollState {
  return { remainderX: 0, remainderY: 0 };
}

const PIXELS_PER_SCROLL_STEP = 60;
const PAGE_SCROLL_STEPS = 6;

export function normalizeScroll(
  deltaX: number,
  deltaY: number,
  deltaMode: number,
  state: ScrollState,
): { dx: number; dy: number } {
  let rawX: number, rawY: number;
  switch (deltaMode) {
    case 0: // DOM_DELTA_PIXEL
      rawX = deltaX;
      rawY = deltaY;
      break;
    case 1: // DOM_DELTA_LINE
      rawX = deltaX * PIXELS_PER_SCROLL_STEP;
      rawY = deltaY * PIXELS_PER_SCROLL_STEP;
      break;
    case 2: // DOM_DELTA_PAGE
      rawX = deltaX * PIXELS_PER_SCROLL_STEP * PAGE_SCROLL_STEPS;
      rawY = deltaY * PIXELS_PER_SCROLL_STEP * PAGE_SCROLL_STEPS;
      break;
    default:
      rawX = deltaX;
      rawY = deltaY;
  }

  state.remainderX += rawX;
  state.remainderY += rawY;

  const dx = Math.trunc(state.remainderX / PIXELS_PER_SCROLL_STEP);
  const dy = -Math.trunc(state.remainderY / PIXELS_PER_SCROLL_STEP); // invert for protocol convention

  state.remainderX -= dx * PIXELS_PER_SCROLL_STEP;
  state.remainderY -= (-dy) * PIXELS_PER_SCROLL_STEP;

  return { dx, dy };
}
