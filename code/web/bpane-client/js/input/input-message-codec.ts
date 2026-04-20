import { buildModifiers, domCodeToEvdev } from '../input-map.js';

export interface KeyEventMessageInput {
  code: string;
  key: string;
  down: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  altgr?: boolean;
  extended: boolean;
}

function clampI16(value: number): number {
  return Math.max(-32768, Math.min(32767, Math.trunc(value)));
}

export function encodeClipboardTextMessage(text: string): Uint8Array {
  const encoded = new TextEncoder().encode(text);
  const payload = new Uint8Array(5 + encoded.length);
  payload[0] = 0x01; // CLIPBOARD_TEXT
  payload[1] = encoded.length & 0xFF;
  payload[2] = (encoded.length >> 8) & 0xFF;
  payload[3] = (encoded.length >> 16) & 0xFF;
  payload[4] = (encoded.length >> 24) & 0xFF;
  payload.set(encoded, 5);
  return payload;
}

export function encodeMouseMoveMessage(x: number, y: number): Uint8Array {
  const payload = new Uint8Array(5);
  payload[0] = 0x01; // INPUT_MOUSE_MOVE
  payload[1] = x & 0xFF;
  payload[2] = (x >> 8) & 0xFF;
  payload[3] = y & 0xFF;
  payload[4] = (y >> 8) & 0xFF;
  return payload;
}

export function encodeMouseButtonMessage(
  button: number,
  down: boolean,
  x: number,
  y: number,
): Uint8Array {
  const payload = new Uint8Array(7);
  payload[0] = 0x02; // INPUT_MOUSE_BUTTON
  payload[1] = button;
  payload[2] = down ? 1 : 0;
  payload[3] = x & 0xFF;
  payload[4] = (x >> 8) & 0xFF;
  payload[5] = y & 0xFF;
  payload[6] = (y >> 8) & 0xFF;
  return payload;
}

export function encodeScrollMessage(dx: number, dy: number): Uint8Array {
  const payload = new Uint8Array(5);
  const view = new DataView(payload.buffer);
  view.setUint8(0, 0x03); // INPUT_MOUSE_SCROLL
  view.setInt16(1, clampI16(dx), true);
  view.setInt16(3, clampI16(dy), true);
  return payload;
}

export function encodeKeyEventMessage(input: KeyEventMessageInput): Uint8Array | null {
  const keycode = domCodeToEvdev(input.code);
  if (keycode === undefined) {
    return null;
  }

  const modifiers = buildModifiers(
    input.ctrl,
    input.alt,
    input.shift,
    input.meta,
    input.altgr ?? false,
  );

  if (input.extended) {
    const keyChar = input.key.length === 1 ? (input.key.codePointAt(0) ?? 0) : 0;
    const payload = new Uint8Array(11);
    payload[0] = 0x05; // INPUT_KEY_EVENT_EX
    payload[1] = keycode & 0xFF;
    payload[2] = (keycode >> 8) & 0xFF;
    payload[3] = (keycode >> 16) & 0xFF;
    payload[4] = (keycode >> 24) & 0xFF;
    payload[5] = input.down ? 1 : 0;
    payload[6] = modifiers;
    payload[7] = keyChar & 0xFF;
    payload[8] = (keyChar >> 8) & 0xFF;
    payload[9] = (keyChar >> 16) & 0xFF;
    payload[10] = (keyChar >> 24) & 0xFF;
    return payload;
  }

  const payload = new Uint8Array(7);
  payload[0] = 0x04; // INPUT_KEY_EVENT
  payload[1] = keycode & 0xFF;
  payload[2] = (keycode >> 8) & 0xFF;
  payload[3] = (keycode >> 16) & 0xFF;
  payload[4] = (keycode >> 24) & 0xFF;
  payload[5] = input.down ? 1 : 0;
  payload[6] = modifiers;
  return payload;
}

export function encodeLayoutHintMessage(hint: string): Uint8Array {
  const payload = new Uint8Array(33);
  payload[0] = 0x06; // CTRL_KEYBOARD_LAYOUT_INFO
  const bytes = new TextEncoder().encode(hint.slice(0, 31));
  payload.set(bytes, 1);
  return payload;
}
