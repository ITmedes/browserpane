/**
 * NAL unit reassembly from VideoDatagram fragments.
 *
 * Wire format (21+ bytes):
 *   nal_id: u32 LE
 *   fragment_seq: u16 LE
 *   fragment_total: u16 LE
 *   is_keyframe: u8
 *   pts_us: u64 LE
 *   data_len: u32 LE
 *   data: [u8; data_len]
 *   [flags: u8]           -- optional; if present and bit 0 set, 12 bytes of tile info follow
 *   [tile_info: 12 bytes] -- tile_x(2) tile_y(2) tile_w(2) tile_h(2) screen_w(2) screen_h(2)
 */

export interface TileInfo {
  tileX: number;
  tileY: number;
  tileW: number;
  tileH: number;
  screenW: number;
  screenH: number;
}

export interface ReassembledNal {
  data: Uint8Array;
  isKeyframe: boolean;
  ptsUs: number;
  tileInfo: TileInfo | null;
}

interface PendingEntry {
  total: number;
  frags: Map<number, Uint8Array>;
  isKeyframe: boolean;
  ptsUs: number;
  tileInfo: TileInfo | null;
}

const MAX_PENDING = 32;
const MIN_HEADER = 21;
const DEDUP_WINDOW = 128;

/**
 * Stateful NAL reassembler. Feed it VideoDatagram payloads and it emits
 * complete NAL units when all fragments arrive.
 */
export class NalReassembler {
  private pending = new Map<number, PendingEntry>();
  /** Recently completed nal_ids for deduplication (datagram + reliable stream). */
  private completed = new Set<number>();
  private completedRing: number[] = new Array(DEDUP_WINDOW).fill(-1);
  private completedIdx = 0;

  private markCompleted(nalId: number): void {
    this.completed.add(nalId);
    // Evict the oldest entry in the ring
    const oldId = this.completedRing[this.completedIdx];
    if (oldId !== -1) {
      this.completed.delete(oldId);
    }
    this.completedRing[this.completedIdx] = nalId;
    this.completedIdx = (this.completedIdx + 1) % DEDUP_WINDOW;
  }

  /**
   * Feed a raw VideoDatagram payload. Returns a complete NAL or null.
   */
  push(payload: Uint8Array): ReassembledNal | null {
    if (payload.length < MIN_HEADER) return null;

    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const nalId = view.getUint32(0, true);
    const fragSeq = view.getUint16(4, true);
    const fragTotal = view.getUint16(6, true);
    const isKeyframe = view.getUint8(8) !== 0;
    const ptsUs = Number(view.getBigUint64(9, true));
    const dataLen = view.getUint32(17, true);
    const data = payload.slice(21, 21 + dataLen);

    // Deduplicate: skip NALs already delivered (from datagram or reliable)
    if (this.completed.has(nalId)) return null;

    // Parse optional tile info
    const tileInfo = parseTileInfo(payload, 21 + dataLen);

    // Single-fragment NAL — no reassembly needed
    if (fragTotal === 1) {
      this.markCompleted(nalId);
      return { data, isKeyframe, ptsUs, tileInfo };
    }

    if (!this.pending.has(nalId)) {
      this.pending.set(nalId, {
        total: fragTotal, frags: new Map(), isKeyframe, ptsUs, tileInfo,
      });
    }
    const entry = this.pending.get(nalId)!;
    entry.frags.set(fragSeq, data);

    if (entry.frags.size === entry.total) {
      this.pending.delete(nalId);
      this.markCompleted(nalId);
      let totalLen = 0;
      for (const d of entry.frags.values()) totalLen += d.length;
      const assembled = new Uint8Array(totalLen);
      let offset = 0;
      for (let i = 0; i < entry.total; i++) {
        const part = entry.frags.get(i);
        if (!part) return null; // gap — discard
        assembled.set(part, offset);
        offset += part.length;
      }
      return { data: assembled, isKeyframe: entry.isKeyframe, ptsUs: entry.ptsUs, tileInfo: entry.tileInfo };
    }

    // Evict oldest if too many pending
    if (this.pending.size > MAX_PENDING) {
      const oldest = this.pending.keys().next().value;
      if (oldest !== undefined) this.pending.delete(oldest);
    }

    return null;
  }

  get pendingCount(): number {
    return this.pending.size;
  }
}

/**
 * Parse optional tile info from VideoDatagram wire bytes.
 */
export function parseTileInfo(payload: Uint8Array, flagsOffset: number): TileInfo | null {
  if (payload.length <= flagsOffset) return null;
  const flags = payload[flagsOffset];
  if ((flags & 0x01) === 0) return null;
  if (payload.length < flagsOffset + 1 + 12) return null;
  const tv = new DataView(payload.buffer, payload.byteOffset + flagsOffset + 1, 12);
  return {
    tileX: tv.getUint16(0, true),
    tileY: tv.getUint16(2, true),
    tileW: tv.getUint16(4, true),
    tileH: tv.getUint16(6, true),
    screenW: tv.getUint16(8, true),
    screenH: tv.getUint16(10, true),
  };
}

/**
 * Extract the H.264 NAL unit type from an Annex-B byte stream.
 * Returns 0 if no start code is found.
 */
export function getNalType(data: Uint8Array): number {
  for (let i = 0; i < data.length - 3; i++) {
    if (data[i] === 0 && data[i + 1] === 0) {
      if (data[i + 2] === 1) return data[i + 3] & 0x1F;
      if (data[i + 2] === 0 && i + 3 < data.length && data[i + 3] === 1)
        return data[i + 4] & 0x1F;
    }
  }
  return 0;
}
