import { describe, expect, it } from 'vitest';
import {
  encodeClipboardTextMessage,
  encodeKeyEventMessage,
  encodeLayoutHintMessage,
  encodeMouseButtonMessage,
  encodeMouseMoveMessage,
  encodeScrollMessage,
} from '../input/input-message-codec.js';

function decodeU32(payload: Uint8Array, offset: number): number {
  return new DataView(payload.buffer, payload.byteOffset, payload.byteLength).getUint32(offset, true);
}

function decodeI16(payload: Uint8Array, offset: number): number {
  return new DataView(payload.buffer, payload.byteOffset, payload.byteLength).getInt16(offset, true);
}

function decodeText(payload: Uint8Array, offset: number): string {
  return new TextDecoder().decode(payload.slice(offset)).replace(/\0+$/, '');
}

describe('input message codec', () => {
  it('encodes mouse move payloads', () => {
    const payload = encodeMouseMoveMessage(258, 513);

    expect([...payload]).toEqual([0x01, 0x02, 0x01, 0x01, 0x02]);
  });

  it('encodes mouse button payloads', () => {
    const payload = encodeMouseButtonMessage(2, true, 320, 240);

    expect([...payload]).toEqual([0x02, 0x02, 0x01, 0x40, 0x01, 0xF0, 0x00]);
  });

  it('clamps scroll payloads to protocol i16 bounds', () => {
    const payload = encodeScrollMessage(40000.9, -40000.9);

    expect(payload[0]).toBe(0x03);
    expect(decodeI16(payload, 1)).toBe(32767);
    expect(decodeI16(payload, 3)).toBe(-32768);
  });

  it('encodes extended key events with modifiers and key chars', () => {
    const payload = encodeKeyEventMessage({
      code: 'KeyA',
      key: 'A',
      down: true,
      ctrl: true,
      alt: false,
      shift: true,
      meta: false,
      altgr: true,
      extended: true,
    });

    expect(payload).not.toBeNull();
    expect(payload).toHaveLength(11);
    expect(payload?.[0]).toBe(0x05);
    expect(decodeU32(payload!, 1)).toBe(30);
    expect(payload?.[5]).toBe(1);
    expect(payload?.[6]).toBe(0x15);
    expect(decodeU32(payload!, 7)).toBe('A'.codePointAt(0));
  });

  it('encodes legacy key events without key chars', () => {
    const payload = encodeKeyEventMessage({
      code: 'KeyV',
      key: 'v',
      down: false,
      ctrl: false,
      alt: true,
      shift: false,
      meta: false,
      altgr: false,
      extended: false,
    });

    expect(payload).not.toBeNull();
    expect(payload).toHaveLength(7);
    expect(payload?.[0]).toBe(0x04);
    expect(decodeU32(payload!, 1)).toBe(47);
    expect(payload?.[5]).toBe(0);
    expect(payload?.[6]).toBe(0x02);
  });

  it('returns null for unmapped key codes', () => {
    const payload = encodeKeyEventMessage({
      code: 'UnknownKey',
      key: '',
      down: true,
      ctrl: false,
      alt: false,
      shift: false,
      meta: false,
      altgr: false,
      extended: true,
    });

    expect(payload).toBeNull();
  });

  it('encodes clipboard text payloads', () => {
    const payload = encodeClipboardTextMessage('hello clipboard');

    expect(payload[0]).toBe(0x01);
    expect(decodeU32(payload, 1)).toBe(15);
    expect(decodeText(payload, 5)).toBe('hello clipboard');
  });

  it('truncates layout hints to the control payload limit', () => {
    const payload = encodeLayoutHintMessage('abcdefghijklmnopqrstuvwxyz1234567890');

    expect(payload).toHaveLength(33);
    expect(payload[0]).toBe(0x06);
    expect(decodeText(payload, 1)).toBe('abcdefghijklmnopqrstuvwxyz12345');
  });
});
