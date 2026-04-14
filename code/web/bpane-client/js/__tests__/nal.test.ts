import { describe, it, expect } from 'vitest';
import { NalReassembler, parseTileInfo, getNalType } from '../nal.js';
import { wireFixture } from './wire-fixtures.js';

/**
 * Build a VideoDatagram wire payload.
 *
 * Wire format:
 *   nal_id: u32 LE
 *   fragment_seq: u16 LE
 *   fragment_total: u16 LE
 *   is_keyframe: u8
 *   pts_us: u64 LE
 *   data_len: u32 LE
 *   data: [u8; data_len]
 *   [flags: u8]
 *   [tile_info: 12 bytes]
 */
function buildDatagram(opts: {
  nalId: number;
  fragSeq: number;
  fragTotal: number;
  isKeyframe: boolean;
  ptsUs?: number;
  data: Uint8Array;
  tileInfo?: { tileX: number; tileY: number; tileW: number; tileH: number; screenW: number; screenH: number };
}): Uint8Array {
  const hasFlags = opts.tileInfo !== undefined;
  const tileLen = opts.tileInfo ? 13 : 0; // 1 (flags) + 12 (tile info)
  const buf = new Uint8Array(21 + opts.data.length + tileLen);
  const view = new DataView(buf.buffer);
  view.setUint32(0, opts.nalId, true);
  view.setUint16(4, opts.fragSeq, true);
  view.setUint16(6, opts.fragTotal, true);
  view.setUint8(8, opts.isKeyframe ? 1 : 0);
  // pts_us as u64 LE
  const pts = opts.ptsUs ?? 0;
  view.setUint32(9, pts & 0xFFFFFFFF, true);
  view.setUint32(13, 0, true); // high 32 bits
  view.setUint32(17, opts.data.length, true);
  buf.set(opts.data, 21);

  if (opts.tileInfo) {
    buf[21 + opts.data.length] = 0x01; // flags: tile info present
    const tv = new DataView(buf.buffer, 21 + opts.data.length + 1, 12);
    tv.setUint16(0, opts.tileInfo.tileX, true);
    tv.setUint16(2, opts.tileInfo.tileY, true);
    tv.setUint16(4, opts.tileInfo.tileW, true);
    tv.setUint16(6, opts.tileInfo.tileH, true);
    tv.setUint16(8, opts.tileInfo.screenW, true);
    tv.setUint16(10, opts.tileInfo.screenH, true);
  }

  return buf;
}

describe('NalReassembler', () => {
  it('passes through single-fragment NAL', () => {
    const nal = new NalReassembler();
    const data = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0xAA]);
    const payload = buildDatagram({
      nalId: 1, fragSeq: 0, fragTotal: 1, isKeyframe: true, data,
    });
    const result = nal.push(payload);
    expect(result).not.toBeNull();
    expect(result!.data).toEqual(data);
    expect(result!.isKeyframe).toBe(true);
    expect(result!.tileInfo).toBeNull();
  });

  it('reassembles two fragments', () => {
    const nal = new NalReassembler();
    const part0 = new Uint8Array([0x00, 0x00, 0x01]);
    const part1 = new Uint8Array([0x65, 0xBB]);

    const d0 = buildDatagram({ nalId: 42, fragSeq: 0, fragTotal: 2, isKeyframe: false, data: part0 });
    const d1 = buildDatagram({ nalId: 42, fragSeq: 1, fragTotal: 2, isKeyframe: false, data: part1 });

    expect(nal.push(d0)).toBeNull();
    const result = nal.push(d1);
    expect(result).not.toBeNull();
    expect(result!.data).toEqual(new Uint8Array([0x00, 0x00, 0x01, 0x65, 0xBB]));
    expect(result!.isKeyframe).toBe(false);
  });

  it('reassembles three fragments in order', () => {
    const nal = new NalReassembler();
    const d0 = buildDatagram({ nalId: 1, fragSeq: 0, fragTotal: 3, isKeyframe: true, data: new Uint8Array([0xAA]) });
    const d1 = buildDatagram({ nalId: 1, fragSeq: 1, fragTotal: 3, isKeyframe: true, data: new Uint8Array([0xBB]) });
    const d2 = buildDatagram({ nalId: 1, fragSeq: 2, fragTotal: 3, isKeyframe: true, data: new Uint8Array([0xCC]) });

    expect(nal.push(d0)).toBeNull();
    expect(nal.push(d1)).toBeNull();
    const result = nal.push(d2);
    expect(result).not.toBeNull();
    expect(result!.data).toEqual(new Uint8Array([0xAA, 0xBB, 0xCC]));
  });

  it('reassembles fragments arriving out of order', () => {
    const nal = new NalReassembler();
    const d0 = buildDatagram({ nalId: 5, fragSeq: 0, fragTotal: 2, isKeyframe: false, data: new Uint8Array([0x11]) });
    const d1 = buildDatagram({ nalId: 5, fragSeq: 1, fragTotal: 2, isKeyframe: false, data: new Uint8Array([0x22]) });

    // Send fragment 1 first
    expect(nal.push(d1)).toBeNull();
    const result = nal.push(d0);
    expect(result).not.toBeNull();
    expect(result!.data).toEqual(new Uint8Array([0x11, 0x22]));
  });

  it('rejects too-short payloads', () => {
    const nal = new NalReassembler();
    expect(nal.push(new Uint8Array(20))).toBeNull(); // < 21 bytes
    expect(nal.push(new Uint8Array(0))).toBeNull();
  });

  it('discards NAL with missing fragment', () => {
    const nal = new NalReassembler();
    // 3 fragments, but skip fragment 1
    const d0 = buildDatagram({ nalId: 10, fragSeq: 0, fragTotal: 3, isKeyframe: false, data: new Uint8Array([0xAA]) });
    const d2 = buildDatagram({ nalId: 10, fragSeq: 2, fragTotal: 3, isKeyframe: false, data: new Uint8Array([0xCC]) });

    expect(nal.push(d0)).toBeNull();
    expect(nal.push(d2)).toBeNull();
    // Entry is still pending (2 of 3 fragments)
    expect(nal.pendingCount).toBe(1);
  });

  it('evicts oldest entry when too many pending', () => {
    const nal = new NalReassembler();
    // Create 33 pending NALs (each missing 1 fragment of 2)
    for (let i = 0; i < 33; i++) {
      const d = buildDatagram({ nalId: i, fragSeq: 0, fragTotal: 2, isKeyframe: false, data: new Uint8Array([i]) });
      nal.push(d);
    }
    // MAX_PENDING = 32, so oldest should be evicted
    expect(nal.pendingCount).toBeLessThanOrEqual(32);
  });

  it('parses tile info from single-fragment NAL', () => {
    const nal = new NalReassembler();
    const data = new Uint8Array([0x00, 0x00, 0x01, 0x65]);
    const payload = buildDatagram({
      nalId: 1, fragSeq: 0, fragTotal: 1, isKeyframe: true, data,
      tileInfo: { tileX: 100, tileY: 200, tileW: 128, tileH: 64, screenW: 1920, screenH: 1080 },
    });
    const result = nal.push(payload);
    expect(result).not.toBeNull();
    expect(result!.tileInfo).not.toBeNull();
    expect(result!.tileInfo!.tileX).toBe(100);
    expect(result!.tileInfo!.tileY).toBe(200);
    expect(result!.tileInfo!.tileW).toBe(128);
    expect(result!.tileInfo!.tileH).toBe(64);
    expect(result!.tileInfo!.screenW).toBe(1920);
    expect(result!.tileInfo!.screenH).toBe(1080);
  });

  it('handles interleaved NALs from different nal_ids', () => {
    const nal = new NalReassembler();
    // NAL 1: 2 fragments
    const n1f0 = buildDatagram({ nalId: 1, fragSeq: 0, fragTotal: 2, isKeyframe: true, data: new Uint8Array([0xAA]) });
    // NAL 2: single fragment
    const n2 = buildDatagram({ nalId: 2, fragSeq: 0, fragTotal: 1, isKeyframe: false, data: new Uint8Array([0xCC]) });
    // NAL 1: second fragment
    const n1f1 = buildDatagram({ nalId: 1, fragSeq: 1, fragTotal: 2, isKeyframe: true, data: new Uint8Array([0xBB]) });

    expect(nal.push(n1f0)).toBeNull();
    const r2 = nal.push(n2);
    expect(r2).not.toBeNull();
    expect(r2!.data).toEqual(new Uint8Array([0xCC]));
    const r1 = nal.push(n1f1);
    expect(r1).not.toBeNull();
    expect(r1!.data).toEqual(new Uint8Array([0xAA, 0xBB]));
  });

  it('reassembles the shared video fixture with tile metadata intact', () => {
    const nal = new NalReassembler();
    const result = nal.push(wireFixture('video_single_fragment_tile'));
    expect(result).not.toBeNull();
    expect(result!.data).toEqual(new Uint8Array([0x00, 0x00, 0x01, 0x65, 0xAA, 0xBB]));
    expect(result!.isKeyframe).toBe(true);
    expect(result!.tileInfo).toEqual({
      tileX: 100,
      tileY: 200,
      tileW: 320,
      tileH: 180,
      screenW: 1920,
      screenH: 1080,
    });
  });
});

describe('parseTileInfo', () => {
  it('returns null when no flags byte', () => {
    const payload = new Uint8Array(21);
    expect(parseTileInfo(payload, 21)).toBeNull();
  });

  it('returns null when flags bit 0 is not set', () => {
    const payload = new Uint8Array(22);
    payload[21] = 0x00; // flags: tile info not present
    expect(parseTileInfo(payload, 21)).toBeNull();
  });

  it('returns null when tile info is truncated', () => {
    const payload = new Uint8Array(25); // flags + only 3 bytes (need 12)
    payload[21] = 0x01;
    expect(parseTileInfo(payload, 21)).toBeNull();
  });

  it('parses valid tile info', () => {
    const payload = new Uint8Array(21 + 1 + 12);
    payload[21] = 0x01; // flags
    const tv = new DataView(payload.buffer, 22, 12);
    tv.setUint16(0, 50, true);   // tileX
    tv.setUint16(2, 100, true);  // tileY
    tv.setUint16(4, 256, true);  // tileW
    tv.setUint16(6, 128, true);  // tileH
    tv.setUint16(8, 1920, true); // screenW
    tv.setUint16(10, 1080, true); // screenH

    const info = parseTileInfo(payload, 21);
    expect(info).not.toBeNull();
    expect(info!.tileX).toBe(50);
    expect(info!.tileY).toBe(100);
    expect(info!.tileW).toBe(256);
    expect(info!.tileH).toBe(128);
    expect(info!.screenW).toBe(1920);
    expect(info!.screenH).toBe(1080);
  });
});

describe('getNalType', () => {
  it('detects IDR (type 5) with 3-byte start code', () => {
    const data = new Uint8Array([0x00, 0x00, 0x01, 0x65]); // 0x65 & 0x1F = 5
    expect(getNalType(data)).toBe(5);
  });

  it('detects SPS (type 7) with 4-byte start code', () => {
    const data = new Uint8Array([0x00, 0x00, 0x00, 0x01, 0x67]); // 0x67 & 0x1F = 7
    expect(getNalType(data)).toBe(7);
  });

  it('detects PPS (type 8)', () => {
    const data = new Uint8Array([0x00, 0x00, 0x01, 0x68]); // 0x68 & 0x1F = 8
    expect(getNalType(data)).toBe(8);
  });

  it('detects non-IDR slice (type 1)', () => {
    const data = new Uint8Array([0x00, 0x00, 0x01, 0x41]); // 0x41 & 0x1F = 1
    expect(getNalType(data)).toBe(1);
  });

  it('detects SEI (type 6)', () => {
    const data = new Uint8Array([0x00, 0x00, 0x01, 0x06]);
    expect(getNalType(data)).toBe(6);
  });

  it('returns 0 for no start code', () => {
    const data = new Uint8Array([0x01, 0x02, 0x03, 0x04]);
    expect(getNalType(data)).toBe(0);
  });

  it('returns 0 for too-short data', () => {
    expect(getNalType(new Uint8Array([0x00, 0x00]))).toBe(0);
    expect(getNalType(new Uint8Array([]))).toBe(0);
  });

  it('finds start code after prefix data', () => {
    const data = new Uint8Array([0xFF, 0xFF, 0x00, 0x00, 0x01, 0x65]);
    expect(getNalType(data)).toBe(5);
  });
});
