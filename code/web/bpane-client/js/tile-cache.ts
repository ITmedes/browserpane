/**
 * Client-side tile cache with LRU eviction.
 *
 * Stores rendered tiles by hash for instant reuse. When the server sends
 * a CacheHit message, the client looks up the hash here instead of
 * re-decoding/re-rendering the tile.
 *
 * Capacity is bounded by both entry count (defense in depth) and total byte
 * size. The byte cap is sized so the default 64x64 RGBA tile case does not
 * undercut the 8192-entry contract used by the host-side sent-hash cache.
 * Eviction removes LRU entries until both limits are satisfied.
 */

import { CH_TILES, parseTileMessage } from './render/tile-message-parser.js';

/** Tile grid configuration from the server. */
export interface TileGridConfig {
  tileSize: number;
  cols: number;
  rows: number;
  screenW: number;
  screenH: number;
}

/** A single tile update command from the server. */
export type TileCommand =
  | { type: 'grid-config'; config: TileGridConfig }
  | { type: 'cache-hit'; col: number; row: number; hash: bigint }
  | { type: 'fill'; col: number; row: number; rgba: number }
  | { type: 'qoi'; col: number; row: number; hash: bigint; data: Uint8Array }
  | { type: 'zstd'; col: number; row: number; hash: bigint; data: Uint8Array }
  | { type: 'video-region'; x: number; y: number; w: number; h: number }
  | { type: 'batch-end'; frameSeq: number }
  | { type: 'scroll-copy'; dx: number; dy: number; regionTop: number; regionBottom: number; regionRight: number }
  | { type: 'grid-offset'; offsetX: number; offsetY: number }
  | { type: 'tile-draw-mode'; applyOffset: boolean }
  | {
    type: 'scroll-stats';
    scrollBatchesTotal: number;
    scrollFullFallbacksTotal: number;
    scrollPotentialTilesTotal: number;
    scrollSavedTilesTotal: number;
    scrollNonQuantizedFallbacksTotal: number;
    scrollResidualFullRepaintsTotal: number;
    scrollResidualInteriorLimitFallbacksTotal: number;
    scrollResidualLowSavedRatioFallbacksTotal: number;
    scrollResidualLargeRowShiftFallbacksTotal: number;
    scrollResidualOtherFallbacksTotal: number;
    scrollZeroSavedBatchesTotal: number;
    scrollSplitRegionBatchesTotal: number;
    scrollStickyBandBatchesTotal: number;
    scrollChromeTilesTotal: number;
    scrollExposedStripTilesTotal: number;
    scrollInteriorResidualTilesTotal: number;
    scrollEdgeStripResidualTilesTotal: number;
    scrollSmallEdgeStripResidualTilesTotal: number;
    scrollSmallEdgeStripResidualRowsTotal: number;
    scrollSmallEdgeStripResidualAreaPxTotal: number;
    hostSentHashEntries: number;
    hostSentHashEvictionsTotal: number;
    hostCacheMissReportsTotal: number;
  };

const DEFAULT_MAX_ENTRIES = 8192;
const DEFAULT_MAX_BYTES = DEFAULT_MAX_ENTRIES * 64 * 64 * 4; // 128 MB at default tile size
export type CachedTile = ImageBitmap | ImageData;

export class TileCache {
  // Single Map serves as both storage and LRU order.
  // JS Map preserves insertion order; delete+set moves to end in O(1).
  private cache = new Map<bigint, CachedTile>();
  private maxEntries: number;
  private byteSize = 0;

  /** Stats for monitoring. */
  hits = 0;
  misses = 0;
  evictions = 0;

  constructor(maxEntries = DEFAULT_MAX_ENTRIES, private maxBytes = DEFAULT_MAX_BYTES) {
    this.maxEntries = maxEntries;
  }

  /** Estimate the byte footprint of a tile. */
  private tileBytes(tile: CachedTile): number {
    if ('close' in tile && typeof tile.close === 'function') {
      // ImageBitmap — estimate from dimensions
      const bmp = tile as ImageBitmap;
      return bmp.width * bmp.height * 4;
    }
    // ImageData — exact size
    return (tile as ImageData).data.byteLength;
  }

  /** Total byte size of all cached tiles. */
  get bytes(): number {
    return this.byteSize;
  }

  /** Look up a tile by hash. Returns null on miss. */
  get(hash: bigint): CachedTile | null {
    const bmp = this.cache.get(hash);
    if (bmp) {
      this.hits++;
      // Move to end (most recently used) — O(1) via delete + set
      this.cache.delete(hash);
      this.cache.set(hash, bmp);
      return bmp;
    }
    this.misses++;
    return null;
  }

  /** Check if a hash is present without mutating hit/miss counters or LRU order. */
  has(hash: bigint): boolean {
    return this.cache.has(hash);
  }

  /** Store a decoded tile by hash. Evicts oldest if at capacity. */
  set(hash: bigint, tile: CachedTile): void {
    if (this.cache.has(hash)) {
      // Update existing — subtract old byte cost, close bitmap, then replace.
      const old = this.cache.get(hash)!;
      this.byteSize -= this.tileBytes(old);
      if ('close' in old && typeof old.close === 'function') {
        old.close();
      }
      // delete + set to move to end of insertion order
      this.cache.delete(hash);
      this.cache.set(hash, tile);
      this.byteSize += this.tileBytes(tile);
      return;
    }

    // Evict oldest (first in insertion order) while over either limit.
    // Check byteSize >= maxBytes so that adding the incoming tile does not
    // push the total above the cap (the new tile's bytes are added after the loop).
    while (this.cache.size >= this.maxEntries || this.byteSize >= this.maxBytes) {
      const oldest = this.cache.keys().next().value;
      if (oldest === undefined) break;
      const old = this.cache.get(oldest);
      if (old) {
        this.byteSize -= this.tileBytes(old);
        if ('close' in old && typeof old.close === 'function') {
          old.close();
        }
      }
      this.cache.delete(oldest);
      this.evictions++;
    }

    this.cache.set(hash, tile);
    this.byteSize += this.tileBytes(tile);
  }

  /** Number of cached entries. */
  get size(): number {
    return this.cache.size;
  }

  /** Clear all cached tiles. */
  clear(): void {
    for (const tile of this.cache.values()) {
      if ('close' in tile && typeof tile.close === 'function') {
        tile.close();
      }
    }
    this.cache.clear();
    this.byteSize = 0;
  }

  /** Hit rate as a percentage (0–100). */
  get hitRate(): number {
    const total = this.hits + this.misses;
    return total === 0 ? 0 : (this.hits / total) * 100;
  }
}

export { CH_TILES, parseTileMessage };
