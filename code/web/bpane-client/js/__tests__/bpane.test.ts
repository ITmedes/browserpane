import { describe, it, expect } from 'vitest';
import { inferLayoutHint, inferLayoutName } from '../bpane.js';
import { encodeFrame, parseFrames, CH_CONTROL, CH_CLIPBOARD, CH_INPUT, CH_CURSOR } from '../protocol.js';

describe('inferLayoutName', () => {
  it('detects US QWERTY layout', () => {
    const map = new Map<string, string>([
      ['KeyQ', 'q'], ['KeyW', 'w'], ['KeyY', 'y'], ['KeyZ', 'z'],
    ]);
    expect(inferLayoutName(map)).toBe('us');
  });

  it('detects French AZERTY layout', () => {
    const map = new Map<string, string>([
      ['KeyQ', 'a'], ['KeyW', 'z'], ['KeyY', 'y'], ['KeyZ', 'w'],
    ]);
    expect(inferLayoutName(map)).toBe('fr');
  });

  it('detects German QWERTZ layout', () => {
    const map = new Map<string, string>([
      ['KeyQ', 'q'], ['KeyW', 'w'], ['KeyY', 'z'], ['KeyZ', 'y'],
    ]);
    expect(inferLayoutName(map)).toBe('de');
  });

  it('returns empty string for unknown layout', () => {
    const map = new Map<string, string>([
      ['KeyQ', 'x'], ['KeyW', 'x'], ['KeyY', 'x'], ['KeyZ', 'x'],
    ]);
    expect(inferLayoutName(map)).toBe('');
  });

  it('returns empty string for empty map', () => {
    expect(inferLayoutName(new Map())).toBe('');
  });
});

describe('inferLayoutHint', () => {
  it('adds physical and OS metadata to the layout hint', () => {
    const map = new Map<string, string>([
      ['KeyQ', 'q'], ['KeyW', 'w'], ['KeyY', 'y'], ['KeyZ', 'z'],
    ]);
    expect(inferLayoutHint(map)).toBe('us-ansi-win');
  });
});

describe('control message encoding', () => {
  it('encodes ResolutionRequest', () => {
    const width = 1920;
    const height = 1080;
    const payload = new Uint8Array(5);
    payload[0] = 0x01; // CTRL_RESOLUTION_REQUEST
    payload[1] = width & 0xFF;
    payload[2] = (width >> 8) & 0xFF;
    payload[3] = height & 0xFF;
    payload[4] = (height >> 8) & 0xFF;

    const frame = encodeFrame(CH_CONTROL, payload);
    const [frames] = parseFrames(frame);
    expect(frames.length).toBe(1);
    const p = frames[0].payload;
    expect(p[0]).toBe(0x01);
    const w = p[1] | (p[2] << 8);
    const h = p[3] | (p[4] << 8);
    expect(w).toBe(1920);
    expect(h).toBe(1080);
  });

  it('encodes Ping message', () => {
    const payload = new Uint8Array(13);
    const view = new DataView(payload.buffer);
    payload[0] = 0x04; // Ping
    view.setUint32(1, 42, true); // seq
    const now = BigInt(Date.now());
    view.setUint32(5, Number(now & 0xFFFFFFFFn), true);
    view.setUint32(9, Number((now >> 32n) & 0xFFFFFFFFn), true);

    const frame = encodeFrame(CH_CONTROL, payload);
    const [frames] = parseFrames(frame);
    expect(frames.length).toBe(1);
    const p = frames[0].payload;
    expect(p[0]).toBe(0x04);
    const seqView = new DataView(p.buffer, p.byteOffset + 1, 4);
    expect(seqView.getUint32(0, true)).toBe(42);
  });

  it('encodes Pong from Ping payload', () => {
    // Simulate receiving a Ping and creating a Pong
    const ping = new Uint8Array(13);
    ping[0] = 0x04;
    const pingView = new DataView(ping.buffer);
    pingView.setUint32(1, 7, true); // seq = 7
    pingView.setUint32(5, 12345, true); // timestamp low
    pingView.setUint32(9, 0, true); // timestamp high

    // Create Pong response
    const pong = new Uint8Array(13);
    pong[0] = 0x05; // Pong tag
    pong.set(ping.slice(1, 13), 1);

    expect(pong[0]).toBe(0x05);
    const pongView = new DataView(pong.buffer, 1, 4);
    expect(pongView.getUint32(0, true)).toBe(7); // same seq
  });
});

describe('clipboard message encoding', () => {
  it('encodes clipboard text', () => {
    const text = 'hello world';
    const encoded = new TextEncoder().encode(text);
    const payload = new Uint8Array(5 + encoded.length);
    payload[0] = 0x01; // CLIPBOARD_TEXT
    payload[1] = encoded.length & 0xFF;
    payload[2] = (encoded.length >> 8) & 0xFF;
    payload[3] = (encoded.length >> 16) & 0xFF;
    payload[4] = (encoded.length >> 24) & 0xFF;
    payload.set(encoded, 5);

    const frame = encodeFrame(CH_CLIPBOARD, payload);
    const [frames] = parseFrames(frame);
    expect(frames.length).toBe(1);

    const p = frames[0].payload;
    expect(p[0]).toBe(0x01);
    const len = p[1] | (p[2] << 8) | (p[3] << 16) | (p[4] << 24);
    expect(len).toBe(text.length);
    const decoded = new TextDecoder().decode(p.slice(5, 5 + len));
    expect(decoded).toBe(text);
  });

  it('handles empty clipboard text', () => {
    const text = '';
    const encoded = new TextEncoder().encode(text);
    const payload = new Uint8Array(5);
    payload[0] = 0x01;
    // length = 0

    const frame = encodeFrame(CH_CLIPBOARD, payload);
    const [frames] = parseFrames(frame);
    expect(frames[0].payload[0]).toBe(0x01);
    const len = frames[0].payload[1] | (frames[0].payload[2] << 8);
    expect(len).toBe(0);
  });
});

describe('cursor message parsing', () => {
  it('parses CursorMove', () => {
    const payload = new Uint8Array(5);
    const view = new DataView(payload.buffer);
    view.setUint8(0, 0x01); // CursorMove tag
    view.setUint16(1, 500, true); // x
    view.setUint16(3, 300, true); // y

    expect(payload[0]).toBe(0x01);
    const x = view.getUint16(1, true);
    const y = view.getUint16(3, true);
    expect(x).toBe(500);
    expect(y).toBe(300);
  });

  it('parses CursorShape header', () => {
    const width = 32;
    const height = 32;
    const hotspotX = 4;
    const hotspotY = 4;
    const dataLen = width * height * 4;
    const payload = new Uint8Array(11 + dataLen);
    const view = new DataView(payload.buffer);
    view.setUint8(0, 0x02); // CursorShape tag
    view.setUint16(1, width, true);
    view.setUint16(3, height, true);
    view.setUint8(5, hotspotX);
    view.setUint8(6, hotspotY);
    view.setUint32(7, dataLen, true);

    expect(payload[0]).toBe(0x02);
    expect(view.getUint16(1, true)).toBe(32);
    expect(view.getUint16(3, true)).toBe(32);
    expect(view.getUint8(5)).toBe(4);
    expect(view.getUint8(6)).toBe(4);
    expect(view.getUint32(7, true)).toBe(4096);
  });
});

describe('input message encoding', () => {
  it('encodes MouseMove', () => {
    const x = 512;
    const y = 384;
    const payload = new Uint8Array(5);
    payload[0] = 0x01;
    payload[1] = x & 0xFF;
    payload[2] = (x >> 8) & 0xFF;
    payload[3] = y & 0xFF;
    payload[4] = (y >> 8) & 0xFF;

    const frame = encodeFrame(CH_INPUT, payload);
    const [frames] = parseFrames(frame);
    const p = frames[0].payload;
    expect(p[0]).toBe(0x01);
    const px = p[1] | (p[2] << 8);
    const py = p[3] | (p[4] << 8);
    expect(px).toBe(512);
    expect(py).toBe(384);
  });

  it('encodes MouseButton', () => {
    const payload = new Uint8Array(7);
    payload[0] = 0x02;
    payload[1] = 0; // left button
    payload[2] = 1; // down
    payload[3] = 100 & 0xFF;
    payload[4] = (100 >> 8) & 0xFF;
    payload[5] = 200 & 0xFF;
    payload[6] = (200 >> 8) & 0xFF;

    const frame = encodeFrame(CH_INPUT, payload);
    const [frames] = parseFrames(frame);
    const p = frames[0].payload;
    expect(p[0]).toBe(0x02);
    expect(p[1]).toBe(0); // button
    expect(p[2]).toBe(1); // down
  });

  it('encodes MouseScroll with signed values', () => {
    const payload = new Uint8Array(5);
    const view = new DataView(payload.buffer);
    view.setUint8(0, 0x03);
    view.setInt16(1, -3, true); // scroll left
    view.setInt16(3, 2, true);  // scroll up

    const frame = encodeFrame(CH_INPUT, payload);
    const [frames] = parseFrames(frame);
    const p = frames[0].payload;
    const pv = new DataView(p.buffer, p.byteOffset, p.byteLength);
    expect(pv.getInt16(1, true)).toBe(-3);
    expect(pv.getInt16(3, true)).toBe(2);
  });

  it('encodes KeyEvent', () => {
    const keycode = 28; // Enter
    const payload = new Uint8Array(7);
    payload[0] = 0x04;
    payload[1] = keycode & 0xFF;
    payload[2] = (keycode >> 8) & 0xFF;
    payload[3] = (keycode >> 16) & 0xFF;
    payload[4] = (keycode >> 24) & 0xFF;
    payload[5] = 1; // down
    payload[6] = 0x01; // Ctrl

    const frame = encodeFrame(CH_INPUT, payload);
    const [frames] = parseFrames(frame);
    const p = frames[0].payload;
    expect(p[0]).toBe(0x04);
    const kc = p[1] | (p[2] << 8) | (p[3] << 16) | (p[4] << 24);
    expect(kc).toBe(28);
    expect(p[5]).toBe(1);
    expect(p[6]).toBe(0x01);
  });

  it('encodes KeyEventEx with key_char', () => {
    const keycode = 30; // KeyA
    const keyChar = 'a'.codePointAt(0)!;
    const payload = new Uint8Array(11);
    payload[0] = 0x05;
    payload[1] = keycode & 0xFF;
    payload[2] = (keycode >> 8) & 0xFF;
    payload[3] = (keycode >> 16) & 0xFF;
    payload[4] = (keycode >> 24) & 0xFF;
    payload[5] = 1; // down
    payload[6] = 0x04; // Shift
    payload[7] = keyChar & 0xFF;
    payload[8] = (keyChar >> 8) & 0xFF;
    payload[9] = (keyChar >> 16) & 0xFF;
    payload[10] = (keyChar >> 24) & 0xFF;

    const frame = encodeFrame(CH_INPUT, payload);
    const [frames] = parseFrames(frame);
    const p = frames[0].payload;
    expect(p[0]).toBe(0x05);
    const kc = p[1] | (p[2] << 8) | (p[3] << 16) | (p[4] << 24);
    expect(kc).toBe(30);
    expect(p[5]).toBe(1);
    expect(p[6]).toBe(0x04);
    const kch = p[7] | (p[8] << 8) | (p[9] << 16) | (p[10] << 24);
    expect(kch).toBe(97); // 'a'
  });
});

describe('ResolutionAck parsing', () => {
  it('correctly parses width and height', () => {
    const payload = new Uint8Array(5);
    payload[0] = 0x02; // ResolutionAck tag
    payload[1] = 0x80; payload[2] = 0x07; // 1920
    payload[3] = 0x38; payload[4] = 0x04; // 1080

    const width = payload[1] | (payload[2] << 8);
    const height = payload[3] | (payload[4] << 8);
    expect(width).toBe(1920);
    expect(height).toBe(1080);
  });
});

describe('SessionReady parsing', () => {
  it('parses flags byte', () => {
    const payload = new Uint8Array(3);
    payload[0] = 0x03; // SessionReady tag
    payload[1] = 1;    // version
    payload[2] = 0x20; // flags (KEYBOARD_LAYOUT bit set)

    const flags = payload[2];
    const supportsKeyEventEx = (flags & 0x20) !== 0;
    expect(supportsKeyEventEx).toBe(true);
  });

  it('detects when KeyEventEx is not supported', () => {
    const payload = new Uint8Array(3);
    payload[0] = 0x03;
    payload[1] = 1;
    payload[2] = 0x00; // no flags

    const flags = payload[2];
    const supportsKeyEventEx = (flags & 0x20) !== 0;
    expect(supportsKeyEventEx).toBe(false);
  });
});

describe('audio frame header', () => {
  it('correctly encodes mic frame header', () => {
    const seq = 42;
    const timestampUs = seq * 20000;
    const pcmLen = 1920; // 960 mono samples * 2 bytes

    const header = new Uint8Array(16);
    const view = new DataView(header.buffer);
    view.setUint32(0, seq, true);
    view.setUint32(4, timestampUs & 0xFFFFFFFF, true);
    view.setUint32(8, Math.floor(timestampUs / 0x100000000), true);
    view.setUint32(12, pcmLen, true);

    // Read back
    expect(view.getUint32(0, true)).toBe(42);
    expect(view.getUint32(4, true)).toBe(840000);
    expect(view.getUint32(12, true)).toBe(1920);
  });
});
