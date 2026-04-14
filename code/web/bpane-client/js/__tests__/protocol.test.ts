import { describe, it, expect } from 'vitest';
import {
  encodeFrame, parseFrames,
  FRAME_HEADER_SIZE, MAX_FRAME_PAYLOAD,
  CH_VIDEO, CH_AUDIO_OUT, CH_AUDIO_IN, CH_VIDEO_IN,
  CH_INPUT, CH_CURSOR, CH_CLIPBOARD, CH_FILE_UP, CH_FILE_DOWN, CH_CONTROL,
  AUDIO_FRAME_HEADER_SIZE,
} from '../protocol.js';
import { wireFixture } from './wire-fixtures.js';

describe('protocol constants', () => {
  it('has correct channel IDs', () => {
    expect(CH_VIDEO).toBe(0x01);
    expect(CH_AUDIO_OUT).toBe(0x02);
    expect(CH_AUDIO_IN).toBe(0x03);
    expect(CH_VIDEO_IN).toBe(0x04);
    expect(CH_INPUT).toBe(0x05);
    expect(CH_CURSOR).toBe(0x06);
    expect(CH_CLIPBOARD).toBe(0x07);
    expect(CH_FILE_UP).toBe(0x08);
    expect(CH_FILE_DOWN).toBe(0x09);
    expect(CH_CONTROL).toBe(0x0A);
  });

  it('has correct frame header size', () => {
    expect(FRAME_HEADER_SIZE).toBe(5);
  });

  it('has correct audio frame header size', () => {
    expect(AUDIO_FRAME_HEADER_SIZE).toBe(16);
  });
});

describe('encodeFrame', () => {
  it('encodes an empty payload', () => {
    const frame = encodeFrame(CH_CONTROL, new Uint8Array(0));
    expect(frame.length).toBe(FRAME_HEADER_SIZE);
    expect(frame[0]).toBe(CH_CONTROL);
    // length = 0 in LE
    expect(frame[1]).toBe(0);
    expect(frame[2]).toBe(0);
    expect(frame[3]).toBe(0);
    expect(frame[4]).toBe(0);
  });

  it('encodes a small payload', () => {
    const payload = new Uint8Array([0x01, 0x02, 0x03]);
    const frame = encodeFrame(CH_INPUT, payload);
    expect(frame.length).toBe(FRAME_HEADER_SIZE + 3);
    expect(frame[0]).toBe(CH_INPUT);
    // length = 3 in LE
    expect(frame[1]).toBe(3);
    expect(frame[2]).toBe(0);
    expect(frame[3]).toBe(0);
    expect(frame[4]).toBe(0);
    // payload
    expect(frame[5]).toBe(0x01);
    expect(frame[6]).toBe(0x02);
    expect(frame[7]).toBe(0x03);
  });

  it('encodes length in little-endian', () => {
    const payload = new Uint8Array(256);
    const frame = encodeFrame(CH_VIDEO, payload);
    expect(frame[1]).toBe(0x00); // 256 & 0xFF
    expect(frame[2]).toBe(0x01); // (256 >> 8) & 0xFF
    expect(frame[3]).toBe(0x00);
    expect(frame[4]).toBe(0x00);
  });

  it('preserves payload data exactly', () => {
    const payload = new Uint8Array(100);
    for (let i = 0; i < 100; i++) payload[i] = i;
    const frame = encodeFrame(CH_CURSOR, payload);
    for (let i = 0; i < 100; i++) {
      expect(frame[FRAME_HEADER_SIZE + i]).toBe(i);
    }
  });
});

describe('parseFrames', () => {
  it('parses a single complete frame', () => {
    const payload = new Uint8Array([0xAA, 0xBB]);
    const frame = encodeFrame(CH_INPUT, payload);
    const [frames, remaining] = parseFrames(frame);
    expect(frames.length).toBe(1);
    expect(frames[0].channelId).toBe(CH_INPUT);
    expect(frames[0].payload).toEqual(payload);
    expect(remaining.length).toBe(0);
  });

  it('parses multiple frames in one buffer', () => {
    const f1 = encodeFrame(CH_INPUT, new Uint8Array([1]));
    const f2 = encodeFrame(CH_CONTROL, new Uint8Array([2, 3]));
    const buf = new Uint8Array(f1.length + f2.length);
    buf.set(f1);
    buf.set(f2, f1.length);

    const [frames, remaining] = parseFrames(buf);
    expect(frames.length).toBe(2);
    expect(frames[0].channelId).toBe(CH_INPUT);
    expect(frames[0].payload).toEqual(new Uint8Array([1]));
    expect(frames[1].channelId).toBe(CH_CONTROL);
    expect(frames[1].payload).toEqual(new Uint8Array([2, 3]));
    expect(remaining.length).toBe(0);
  });

  it('returns remaining bytes for incomplete frames', () => {
    const frame = encodeFrame(CH_VIDEO, new Uint8Array([1, 2, 3]));
    // Truncate: only send header + 1 byte of 3
    const truncated = frame.slice(0, FRAME_HEADER_SIZE + 1);
    const [frames, remaining] = parseFrames(truncated);
    expect(frames.length).toBe(0);
    expect(remaining.length).toBe(truncated.length);
  });

  it('parses complete frame and keeps partial remainder', () => {
    const f1 = encodeFrame(CH_INPUT, new Uint8Array([0xAA]));
    const f2 = encodeFrame(CH_CONTROL, new Uint8Array([0xBB, 0xCC]));
    const partial = f2.slice(0, 3); // only 3 bytes of f2's header
    const buf = new Uint8Array(f1.length + partial.length);
    buf.set(f1);
    buf.set(partial, f1.length);

    const [frames, remaining] = parseFrames(buf);
    expect(frames.length).toBe(1);
    expect(frames[0].channelId).toBe(CH_INPUT);
    expect(remaining.length).toBe(3);
  });

  it('handles empty buffer', () => {
    const [frames, remaining] = parseFrames(new Uint8Array(0));
    expect(frames.length).toBe(0);
    expect(remaining.length).toBe(0);
  });

  it('handles buffer shorter than header', () => {
    const [frames, remaining] = parseFrames(new Uint8Array([0x01, 0x02]));
    expect(frames.length).toBe(0);
    expect(remaining.length).toBe(2);
  });

  it('throws on frame exceeding MAX_FRAME_PAYLOAD', () => {
    const bad = new Uint8Array(FRAME_HEADER_SIZE);
    bad[0] = CH_VIDEO;
    // length = MAX_FRAME_PAYLOAD + 1 in LE
    const len = MAX_FRAME_PAYLOAD + 1;
    bad[1] = len & 0xFF;
    bad[2] = (len >> 8) & 0xFF;
    bad[3] = (len >> 16) & 0xFF;
    bad[4] = (len >> 24) & 0xFF;

    expect(() => parseFrames(bad)).toThrow('frame payload too large');
  });

  it('roundtrips encode/parse for various sizes', () => {
    const sizes = [0, 1, 127, 128, 255, 256, 1000, 4096];
    for (const size of sizes) {
      const payload = new Uint8Array(size);
      for (let i = 0; i < size; i++) payload[i] = i & 0xFF;
      const frame = encodeFrame(CH_CLIPBOARD, payload);
      const [frames, remaining] = parseFrames(frame);
      expect(frames.length).toBe(1);
      expect(frames[0].channelId).toBe(CH_CLIPBOARD);
      expect(frames[0].payload).toEqual(payload);
      expect(remaining.length).toBe(0);
    }
  });

  // Fix 6: length field with high bit set must not produce negative via signed << 24
  it('rejects frame with high-bit length as too large (not negative)', () => {
    // Craft a frame header where byte[4] (the MSB of the LE u32 length) has bit 7 set.
    // Length = 0x80_00_00_01 = 2147483649 (> MAX_FRAME_PAYLOAD = 16 MiB)
    // Before fix: (0x80 << 24) produced -2147483648 (signed), which would
    // pass the `length < 0` check and be treated as an error for the wrong reason.
    // After fix: >>> 0 yields 2147483649 (unsigned), correctly caught by > MAX_FRAME_PAYLOAD.
    const bad = new Uint8Array(FRAME_HEADER_SIZE);
    bad[0] = CH_VIDEO;
    bad[1] = 0x01;
    bad[2] = 0x00;
    bad[3] = 0x00;
    bad[4] = 0x80; // high bit set → 0x80000001
    expect(() => parseFrames(bad)).toThrow('frame payload too large');
  });

  it('correctly parses frame length with byte values above 127', () => {
    // Length = 0x00_01_FF_00 = 131072 (128 KiB) — valid, no high bit in MSB
    // All four length bytes exercise bit patterns, byte[2] = 0xFF
    const len = 0x0001FF00;
    const payload = new Uint8Array(len);
    const frame = new Uint8Array(FRAME_HEADER_SIZE + len);
    frame[0] = CH_INPUT;
    frame[1] = (len) & 0xFF;        // 0x00
    frame[2] = (len >> 8) & 0xFF;   // 0xFF
    frame[3] = (len >> 16) & 0xFF;  // 0x01
    frame[4] = (len >> 24) & 0xFF;  // 0x00
    frame.set(payload, FRAME_HEADER_SIZE);
    const [frames, remaining] = parseFrames(frame);
    expect(frames.length).toBe(1);
    expect(frames[0].payload.length).toBe(len);
    expect(remaining.length).toBe(0);
  });

  it('parses the shared control fixture byte-for-byte', () => {
    const [frames, remaining] = parseFrames(wireFixture('control_session_ready'));
    expect(frames).toHaveLength(1);
    expect(frames[0].channelId).toBe(CH_CONTROL);
    expect(frames[0].payload).toEqual(new Uint8Array([0x03, 0x01, 0x35]));
    expect(remaining).toEqual(new Uint8Array(0));
  });

  it('rejects the shared oversized frame fixture', () => {
    expect(() => parseFrames(wireFixture('invalid_frame_oversized_length'))).toThrow(
      'frame payload too large',
    );
  });
});
