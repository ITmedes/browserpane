/**
 * Client-side tile cache with LRU eviction.
 *
 * Stores rendered tiles by hash for instant reuse. When the server sends
 * a CacheHit message, the client looks up the hash here instead of
 * re-decoding/re-rendering the tile.
 *
 * Capacity is bounded by both entry count (defense in depth) and total byte
 * size. The primary limit is 50 MB; the entry count cap of 8192 is a secondary
 * safeguard. Eviction removes LRU entries until both limits are satisfied.
 */

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
  };

const DEFAULT_MAX_ENTRIES = 8192;
const DEFAULT_MAX_BYTES = 50 * 1024 * 1024; // 50 MB
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

// ── Tile channel parser ────────────────────────────────────────────

const CH_TILES = 0x0B;

// Message tags (must match bpane-protocol/src/frame.rs)
const TILE_GRID_CONFIG = 0x01;
const TILE_CACHE_HIT = 0x02;
const TILE_FILL = 0x03;
const TILE_QOI = 0x04;
const TILE_VIDEO_REGION = 0x05;
const TILE_BATCH_END = 0x06;
const TILE_SCROLL_COPY = 0x07;
const TILE_GRID_OFFSET = 0x08;
const TILE_SCROLL_STATS = 0x0A;
const TILE_DRAW_MODE = 0x0B;
const TILE_ZSTD = 0x0C;

/**
 * Parse a Tiles channel payload into a TileCommand.
 * Returns null for unknown/malformed messages.
 */
export function parseTileMessage(payload: Uint8Array): TileCommand | null {
  if (payload.length < 1) return null;
  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  const tag = payload[0];

  switch (tag) {
    case TILE_GRID_CONFIG: {
      if (payload.length < 11) return null;
      return {
        type: 'grid-config',
        config: {
          tileSize: view.getUint16(1, true),
          cols: view.getUint16(3, true),
          rows: view.getUint16(5, true),
          screenW: view.getUint16(7, true),
          screenH: view.getUint16(9, true),
        },
      };
    }
    case TILE_CACHE_HIT: {
      if (payload.length < 13) return null;
      return {
        type: 'cache-hit',
        col: view.getUint16(1, true),
        row: view.getUint16(3, true),
        hash: view.getBigUint64(5, true),
      };
    }
    case TILE_FILL: {
      if (payload.length < 9) return null;
      return {
        type: 'fill',
        col: view.getUint16(1, true),
        row: view.getUint16(3, true),
        rgba: view.getUint32(5, true),
      };
    }
    case TILE_QOI: {
      if (payload.length < 17) return null;
      const dataLen = view.getUint32(13, true);
      if (payload.length < 17 + dataLen) return null;
      return {
        type: 'qoi',
        col: view.getUint16(1, true),
        row: view.getUint16(3, true),
        hash: view.getBigUint64(5, true),
        data: payload.slice(17, 17 + dataLen),
      };
    }
    case TILE_ZSTD: {
      if (payload.length < 17) return null;
      const zstdDataLen = view.getUint32(13, true);
      if (payload.length < 17 + zstdDataLen) return null;
      return {
        type: 'zstd',
        col: view.getUint16(1, true),
        row: view.getUint16(3, true),
        hash: view.getBigUint64(5, true),
        data: payload.slice(17, 17 + zstdDataLen),
      };
    }
    case TILE_VIDEO_REGION: {
      if (payload.length < 9) return null;
      return {
        type: 'video-region',
        x: view.getUint16(1, true),
        y: view.getUint16(3, true),
        w: view.getUint16(5, true),
        h: view.getUint16(7, true),
      };
    }
    case TILE_BATCH_END: {
      if (payload.length < 5) return null;
      return {
        type: 'batch-end',
        frameSeq: view.getUint32(1, true),
      };
    }
    case TILE_SCROLL_COPY: {
      if (payload.length < 11) return null;
      return {
        type: 'scroll-copy',
        dx: view.getInt16(1, true),
        dy: view.getInt16(3, true),
        regionTop: view.getUint16(5, true),
        regionBottom: view.getUint16(7, true),
        regionRight: view.getUint16(9, true),
      };
    }
    case TILE_DRAW_MODE: {
      if (payload.length < 2) return null;
      return {
        type: 'tile-draw-mode',
        applyOffset: payload[1] !== 0,
      };
    }
    case TILE_GRID_OFFSET: {
      if (payload.length < 5) return null;
      return {
        type: 'grid-offset',
        offsetX: view.getInt16(1, true),
        offsetY: view.getInt16(3, true),
      };
    }
    case TILE_SCROLL_STATS: {
      if (payload.length < 17) return null;
      return {
        type: 'scroll-stats',
        scrollBatchesTotal: view.getUint32(1, true),
        scrollFullFallbacksTotal: view.getUint32(5, true),
        scrollPotentialTilesTotal: view.getUint32(9, true),
        scrollSavedTilesTotal: view.getUint32(13, true),
      };
    }
    default:
      return null;
  }
}

export { CH_TILES };
