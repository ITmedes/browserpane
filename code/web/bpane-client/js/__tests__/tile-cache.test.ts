import { describe, it, expect, vi, beforeEach } from 'vitest';
import { parseFrames } from '../protocol.js';
import { TileCache, parseTileMessage, CH_TILES } from '../tile-cache.js';
import type { TileCommand, TileGridConfig } from '../tile-cache.js';
import { wireFixture } from './wire-fixtures.js';

// ── Helper: build a wire-format tile message ────────────────────────

function buildGridConfig(config: TileGridConfig): Uint8Array {
  const buf = new Uint8Array(11);
  const view = new DataView(buf.buffer);
  buf[0] = 0x01; // TILE_GRID_CONFIG
  view.setUint16(1, config.tileSize, true);
  view.setUint16(3, config.cols, true);
  view.setUint16(5, config.rows, true);
  view.setUint16(7, config.screenW, true);
  view.setUint16(9, config.screenH, true);
  return buf;
}

function buildCacheHit(col: number, row: number, hash: bigint): Uint8Array {
  const buf = new Uint8Array(13);
  const view = new DataView(buf.buffer);
  buf[0] = 0x02; // TILE_CACHE_HIT
  view.setUint16(1, col, true);
  view.setUint16(3, row, true);
  view.setBigUint64(5, hash, true);
  return buf;
}

function buildFill(col: number, row: number, rgba: number): Uint8Array {
  const buf = new Uint8Array(9);
  const view = new DataView(buf.buffer);
  buf[0] = 0x03; // TILE_FILL
  view.setUint16(1, col, true);
  view.setUint16(3, row, true);
  view.setUint32(5, rgba, true);
  return buf;
}

function buildQoi(col: number, row: number, hash: bigint, data: Uint8Array): Uint8Array {
  const buf = new Uint8Array(17 + data.length);
  const view = new DataView(buf.buffer);
  buf[0] = 0x04; // TILE_QOI
  view.setUint16(1, col, true);
  view.setUint16(3, row, true);
  view.setBigUint64(5, hash, true);
  view.setUint32(13, data.length, true);
  buf.set(data, 17);
  return buf;
}

function buildVideoRegion(x: number, y: number, w: number, h: number): Uint8Array {
  const buf = new Uint8Array(9);
  const view = new DataView(buf.buffer);
  buf[0] = 0x05; // TILE_VIDEO_REGION
  view.setUint16(1, x, true);
  view.setUint16(3, y, true);
  view.setUint16(5, w, true);
  view.setUint16(7, h, true);
  return buf;
}

function buildBatchEnd(frameSeq: number): Uint8Array {
  const buf = new Uint8Array(5);
  const view = new DataView(buf.buffer);
  buf[0] = 0x06; // TILE_BATCH_END
  view.setUint32(1, frameSeq, true);
  return buf;
}

function buildScrollCopy(dx: number, dy: number, regionTop = 0, regionBottom = 0, regionRight = 0): Uint8Array {
  const buf = new Uint8Array(11);
  const view = new DataView(buf.buffer);
  buf[0] = 0x07; // TILE_SCROLL_COPY
  view.setInt16(1, dx, true);
  view.setInt16(3, dy, true);
  view.setUint16(5, regionTop, true);
  view.setUint16(7, regionBottom, true);
  view.setUint16(9, regionRight, true);
  return buf;
}

function buildTileDrawMode(applyOffset: boolean): Uint8Array {
  const buf = new Uint8Array(2);
  buf[0] = 0x0B; // TILE_DRAW_MODE
  buf[1] = applyOffset ? 1 : 0;
  return buf;
}

function buildScrollStats(
  batches: number,
  fallbacks: number,
  potentialTiles: number,
  savedTiles: number,
  nonQuantizedFallbacks = 0,
  residualFullRepaints = 0,
  zeroSavedBatches = 0,
  hostSentHashEntries = 0,
  hostSentHashEvictionsTotal = 0,
  hostCacheMissReportsTotal = 0,
): Uint8Array {
  const buf = new Uint8Array(41);
  const view = new DataView(buf.buffer);
  buf[0] = 0x0A; // TILE_SCROLL_STATS
  view.setUint32(1, batches, true);
  view.setUint32(5, fallbacks, true);
  view.setUint32(9, potentialTiles, true);
  view.setUint32(13, savedTiles, true);
  view.setUint32(17, nonQuantizedFallbacks, true);
  view.setUint32(21, residualFullRepaints, true);
  view.setUint32(25, zeroSavedBatches, true);
  view.setUint32(29, hostSentHashEntries, true);
  view.setUint32(33, hostSentHashEvictionsTotal, true);
  view.setUint32(37, hostCacheMissReportsTotal, true);
  return buf;
}

// ── Mock ImageBitmap ────────────────────────────────────────────────

function mockBitmap(id = 0): ImageBitmap {
  return { close: vi.fn(), width: 64, height: 64, _id: id } as any;
}

// ── Mock ImageData (not available in jsdom without canvas) ──────────

function mockImageData(width: number, height: number): ImageData {
  return {
    width,
    height,
    data: new Uint8ClampedArray(width * height * 4),
    colorSpace: 'srgb',
  } as ImageData;
}

// ── TileCache tests ─────────────────────────────────────────────────

describe('TileCache', () => {
  let cache: TileCache;

  beforeEach(() => {
    cache = new TileCache(4); // small capacity for testing
  });

  it('starts empty', () => {
    expect(cache.size).toBe(0);
    expect(cache.hits).toBe(0);
    expect(cache.misses).toBe(0);
    expect(cache.evictions).toBe(0);
  });

  it('returns null on cache miss', () => {
    expect(cache.get(123n)).toBeNull();
    expect(cache.misses).toBe(1);
  });

  it('stores and retrieves a bitmap', () => {
    const bmp = mockBitmap();
    cache.set(42n, bmp);
    expect(cache.size).toBe(1);
    expect(cache.get(42n)).toBe(bmp);
    expect(cache.hits).toBe(1);
  });

  it('tracks hit rate correctly', () => {
    cache.set(1n, mockBitmap());
    cache.get(1n); // hit
    cache.get(2n); // miss
    expect(cache.hitRate).toBe(50);
  });

  it('returns 0 hit rate when no lookups', () => {
    expect(cache.hitRate).toBe(0);
  });

  it('evicts oldest entry when at capacity', () => {
    const bmps = [mockBitmap(0), mockBitmap(1), mockBitmap(2), mockBitmap(3), mockBitmap(4)];
    cache.set(10n, bmps[0]);
    cache.set(11n, bmps[1]);
    cache.set(12n, bmps[2]);
    cache.set(13n, bmps[3]);

    expect(cache.size).toBe(4);

    // Insert 5th — should evict hash=10n (oldest)
    cache.set(14n, bmps[4]);
    expect(cache.size).toBe(4);
    expect(cache.get(10n)).toBeNull(); // evicted
    expect(cache.get(14n)).toBe(bmps[4]); // present
    expect(cache.evictions).toBe(1);
    expect(bmps[0].close).toHaveBeenCalled(); // bitmap was closed
  });

  it('LRU: accessing an entry moves it to most recently used', () => {
    cache.set(1n, mockBitmap(1));
    cache.set(2n, mockBitmap(2));
    cache.set(3n, mockBitmap(3));
    cache.set(4n, mockBitmap(4));

    // Access hash=1n to make it most recently used
    cache.get(1n);

    // Insert 5th — should evict hash=2n (oldest untouched)
    cache.set(5n, mockBitmap(5));
    expect(cache.get(1n)).not.toBeNull(); // still present (was refreshed)
    expect(cache.get(2n)).toBeNull(); // evicted (was oldest)
  });

  it('replaces existing entry and closes old bitmap', () => {
    const old = mockBitmap(1);
    const replacement = mockBitmap(2);
    cache.set(1n, old);
    cache.set(1n, replacement);
    expect(cache.size).toBe(1);
    expect(cache.get(1n)).toBe(replacement);
    expect(old.close).toHaveBeenCalled();
  });

  it('clears all entries and closes bitmaps', () => {
    const bmp1 = mockBitmap(1);
    const bmp2 = mockBitmap(2);
    cache.set(1n, bmp1);
    cache.set(2n, bmp2);
    cache.clear();
    expect(cache.size).toBe(0);
    expect(bmp1.close).toHaveBeenCalled();
    expect(bmp2.close).toHaveBeenCalled();
  });

  it('uses default capacity of 8192', () => {
    // Use a very large byte budget so only the entry-count cap is exercised.
    const large = new TileCache(8192, Number.MAX_SAFE_INTEGER);
    // Fill beyond 8192 to check eviction
    for (let i = 0; i < 8193; i++) {
      large.set(BigInt(i), mockBitmap(i));
    }
    expect(large.size).toBe(8192);
    expect(large.evictions).toBe(1);
  });

  it('starts with byteSize of 0', () => {
    expect(cache.bytes).toBe(0);
  });

  it('tracks byte size for ImageBitmap tiles', () => {
    // mockBitmap returns width=64, height=64 => 64*64*4 = 16384 bytes
    cache.set(1n, mockBitmap(1));
    expect(cache.bytes).toBe(64 * 64 * 4);
    cache.set(2n, mockBitmap(2));
    expect(cache.bytes).toBe(2 * 64 * 64 * 4);
  });

  it('tracks byte size for ImageData tiles', () => {
    const imgData = mockImageData(32, 32); // 32*32*4 = 4096 bytes
    cache.set(1n, imgData);
    expect(cache.bytes).toBe(4096);
  });

  it('updates byte size when replacing an existing entry', () => {
    const bmp64 = mockBitmap(1); // 64*64*4 = 16384 bytes
    cache.set(1n, bmp64);
    expect(cache.bytes).toBe(16384);

    const imgData = mockImageData(32, 32); // 4096 bytes
    cache.set(1n, imgData);
    expect(cache.bytes).toBe(4096);
  });

  it('resets byte size to 0 after clear()', () => {
    cache.set(1n, mockBitmap(1));
    cache.set(2n, mockBitmap(2));
    expect(cache.bytes).toBeGreaterThan(0);
    cache.clear();
    expect(cache.bytes).toBe(0);
  });

  it('evicts by byte budget when entry count limit is not yet reached', () => {
    // Each mockBitmap = 64*64*4 = 16384 bytes.
    // maxBytes = 2 * 16384 = 32768. maxEntries set high to not interfere.
    const tileBytes = 64 * 64 * 4; // 16384
    const byteCache = new TileCache(1000, 2 * tileBytes);

    const bmp0 = mockBitmap(0);
    const bmp1 = mockBitmap(1);
    const bmp2 = mockBitmap(2);
    byteCache.set(0n, bmp0);
    byteCache.set(1n, bmp1);
    // At this point byteSize == 2 * tileBytes == maxBytes, no eviction yet.
    expect(byteCache.size).toBe(2);
    expect(byteCache.evictions).toBe(0);

    // Adding a 3rd tile pushes byteSize over the limit before insert,
    // so the eviction loop fires and removes the oldest (hash=0n).
    byteCache.set(2n, bmp2);
    expect(byteCache.evictions).toBe(1);
    expect(byteCache.get(0n)).toBeNull(); // evicted
    expect(byteCache.get(2n)).toBe(bmp2); // present
    expect(byteCache.bytes).toBeLessThanOrEqual(2 * tileBytes);
  });

  it('subtracts bytes from evicted tiles', () => {
    const tileBytes = 64 * 64 * 4;
    // maxBytes = 1 tile, maxEntries high
    const byteCache = new TileCache(1000, tileBytes);
    byteCache.set(0n, mockBitmap(0));
    expect(byteCache.bytes).toBe(tileBytes);

    byteCache.set(1n, mockBitmap(1));
    // After evicting hash=0n and inserting hash=1n, byte size should be tileBytes
    expect(byteCache.bytes).toBe(tileBytes);
    expect(byteCache.evictions).toBe(1);
  });
});

// ── parseTileMessage tests ──────────────────────────────────────────

describe('parseTileMessage', () => {
  it('parses grid-config', () => {
    const msg = parseTileMessage(buildGridConfig({
      tileSize: 64, cols: 20, rows: 12, screenW: 1280, screenH: 768,
    }));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('grid-config');
    if (msg!.type === 'grid-config') {
      expect(msg!.config.tileSize).toBe(64);
      expect(msg!.config.cols).toBe(20);
      expect(msg!.config.rows).toBe(12);
      expect(msg!.config.screenW).toBe(1280);
      expect(msg!.config.screenH).toBe(768);
    }
  });

  it('parses cache-hit', () => {
    const msg = parseTileMessage(buildCacheHit(5, 3, 0xDEADBEEFCAFEn));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('cache-hit');
    if (msg!.type === 'cache-hit') {
      expect(msg!.col).toBe(5);
      expect(msg!.row).toBe(3);
      expect(msg!.hash).toBe(0xDEADBEEFCAFEn);
    }
  });

  it('parses fill', () => {
    const msg = parseTileMessage(buildFill(2, 7, 0xFF0000FF));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('fill');
    if (msg!.type === 'fill') {
      expect(msg!.col).toBe(2);
      expect(msg!.row).toBe(7);
      expect(msg!.rgba).toBe(0xFF0000FF);
    }
  });

  it('parses qoi', () => {
    const qoiData = new Uint8Array([0x71, 0x6f, 0x69, 0x66, 0x00, 0x00]);
    const msg = parseTileMessage(buildQoi(1, 2, 0xABCDn, qoiData));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('qoi');
    if (msg!.type === 'qoi') {
      expect(msg!.col).toBe(1);
      expect(msg!.row).toBe(2);
      expect(msg!.hash).toBe(0xABCDn);
      expect(msg!.data).toEqual(qoiData);
    }
  });

  it('parses video-region', () => {
    const msg = parseTileMessage(buildVideoRegion(128, 64, 640, 480));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('video-region');
    if (msg!.type === 'video-region') {
      expect(msg!.x).toBe(128);
      expect(msg!.y).toBe(64);
      expect(msg!.w).toBe(640);
      expect(msg!.h).toBe(480);
    }
  });

  it('parses batch-end', () => {
    const msg = parseTileMessage(buildBatchEnd(42));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('batch-end');
    if (msg!.type === 'batch-end') {
      expect(msg!.frameSeq).toBe(42);
    }
  });

  it('parses scroll-copy with region bounds', () => {
    const msg = parseTileMessage(buildScrollCopy(0, -128, 100, 600, 1200));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('scroll-copy');
    if (msg!.type === 'scroll-copy') {
      expect(msg!.dx).toBe(0);
      expect(msg!.dy).toBe(-128);
      expect(msg!.regionTop).toBe(100);
      expect(msg!.regionBottom).toBe(600);
      expect(msg!.regionRight).toBe(1200);
    }
  });

  it('parses scroll-copy positive values', () => {
    const msg = parseTileMessage(buildScrollCopy(64, 192, 0, 768, 1280));
    expect(msg).not.toBeNull();
    if (msg!.type === 'scroll-copy') {
      expect(msg!.dx).toBe(64);
      expect(msg!.dy).toBe(192);
      expect(msg!.regionTop).toBe(0);
      expect(msg!.regionBottom).toBe(768);
      expect(msg!.regionRight).toBe(1280);
    }
  });

  it('parses tile-draw-mode true', () => {
    const msg = parseTileMessage(buildTileDrawMode(true));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('tile-draw-mode');
    if (msg!.type === 'tile-draw-mode') {
      expect(msg!.applyOffset).toBe(true);
    }
  });

  it('parses tile-draw-mode false', () => {
    const msg = parseTileMessage(buildTileDrawMode(false));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('tile-draw-mode');
    if (msg!.type === 'tile-draw-mode') {
      expect(msg!.applyOffset).toBe(false);
    }
  });

  it('parses scroll-stats', () => {
    const msg = parseTileMessage(buildScrollStats(11, 2, 1000, 730, 1, 1, 3, 128, 9, 7));
    expect(msg).not.toBeNull();
    expect(msg!.type).toBe('scroll-stats');
    if (msg!.type === 'scroll-stats') {
      expect(msg!.scrollBatchesTotal).toBe(11);
      expect(msg!.scrollFullFallbacksTotal).toBe(2);
      expect(msg!.scrollPotentialTilesTotal).toBe(1000);
      expect(msg!.scrollSavedTilesTotal).toBe(730);
      expect(msg!.scrollNonQuantizedFallbacksTotal).toBe(1);
      expect(msg!.scrollResidualFullRepaintsTotal).toBe(1);
      expect(msg!.scrollZeroSavedBatchesTotal).toBe(3);
      expect(msg!.hostSentHashEntries).toBe(128);
      expect(msg!.hostSentHashEvictionsTotal).toBe(9);
      expect(msg!.hostCacheMissReportsTotal).toBe(7);
    }
  });

  it('returns null for truncated scroll-copy', () => {
    expect(parseTileMessage(new Uint8Array([0x07, 0x00, 0x00]))).toBeNull();
    // 5 bytes is not enough anymore (need 11)
    expect(parseTileMessage(new Uint8Array([0x07, 0x00, 0x00, 0x00, 0x00]))).toBeNull();
  });

  it('returns null for truncated tile-draw-mode', () => {
    expect(parseTileMessage(new Uint8Array([0x0B]))).toBeNull();
  });

  it('returns null for truncated scroll-stats', () => {
    expect(parseTileMessage(new Uint8Array([0x0A, 0x00, 0x00, 0x00]))).toBeNull();
  });

  it('returns null for empty payload', () => {
    expect(parseTileMessage(new Uint8Array(0))).toBeNull();
  });

  it('returns null for unknown tag', () => {
    expect(parseTileMessage(new Uint8Array([0xFF]))).toBeNull();
    expect(parseTileMessage(new Uint8Array([0x00]))).toBeNull();
    expect(parseTileMessage(new Uint8Array([0x09]))).toBeNull();
  });

  it('returns null for truncated grid-config', () => {
    expect(parseTileMessage(new Uint8Array([0x01, 0x00]))).toBeNull();
    expect(parseTileMessage(new Uint8Array([0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]))).toBeNull(); // 10 bytes, need 11
  });

  it('returns null for truncated cache-hit', () => {
    expect(parseTileMessage(new Uint8Array([0x02, 0x00, 0x00, 0x00, 0x00]))).toBeNull(); // 5 bytes, need 13
  });

  it('returns null for truncated fill', () => {
    expect(parseTileMessage(new Uint8Array([0x03, 0x00, 0x00, 0x00]))).toBeNull(); // 4 bytes, need 9
  });

  it('returns null for truncated qoi header', () => {
    expect(parseTileMessage(new Uint8Array([0x04, 0x00, 0x00, 0x00, 0x00]))).toBeNull(); // 5 bytes, need 17
  });

  it('returns null for qoi with insufficient data', () => {
    // Header says 100 bytes of data but only 2 are present
    const buf = new Uint8Array(19);
    const view = new DataView(buf.buffer);
    buf[0] = 0x04;
    view.setUint32(13, 100, true); // dataLen=100
    expect(parseTileMessage(buf)).toBeNull();
  });

  it('returns null for truncated video-region', () => {
    expect(parseTileMessage(new Uint8Array([0x05, 0x00, 0x00, 0x00]))).toBeNull();
  });

  it('returns null for truncated batch-end', () => {
    expect(parseTileMessage(new Uint8Array([0x06, 0x00, 0x00]))).toBeNull();
  });

  it('handles large qoi data', () => {
    const data = new Uint8Array(10000);
    for (let i = 0; i < data.length; i++) data[i] = i & 0xFF;
    const msg = parseTileMessage(buildQoi(0, 0, 999n, data));
    expect(msg).not.toBeNull();
    if (msg!.type === 'qoi') {
      expect(msg!.data.length).toBe(10000);
      expect(msg!.data[0]).toBe(0);
      expect(msg!.data[255]).toBe(255);
    }
  });

  it('handles max u16 col/row values', () => {
    const msg = parseTileMessage(buildFill(0xFFFF, 0xFFFF, 0));
    expect(msg).not.toBeNull();
    if (msg!.type === 'fill') {
      expect(msg!.col).toBe(0xFFFF);
      expect(msg!.row).toBe(0xFFFF);
    }
  });

  it('handles max u64 hash value', () => {
    const maxHash = 0xFFFFFFFFFFFFFFFFn;
    const msg = parseTileMessage(buildCacheHit(0, 0, maxHash));
    expect(msg).not.toBeNull();
    if (msg!.type === 'cache-hit') {
      expect(msg!.hash).toBe(maxHash);
    }
  });

  it('parses the shared tile fixtures', () => {
    const [gridFrames] = parseFrames(wireFixture('tile_grid_config'));
    const grid = parseTileMessage(gridFrames[0].payload);
    expect(grid).toEqual({
      type: 'grid-config',
      config: {
        tileSize: 256,
        cols: 12,
        rows: 8,
        screenW: 1920,
        screenH: 1080,
      },
    });

    const [statsFrames] = parseFrames(wireFixture('tile_scroll_stats'));
    const stats = parseTileMessage(statsFrames[0].payload);
    expect(stats).toEqual({
      type: 'scroll-stats',
      scrollBatchesTotal: 11,
      scrollFullFallbacksTotal: 2,
      scrollPotentialTilesTotal: 1000,
      scrollSavedTilesTotal: 730,
      scrollNonQuantizedFallbacksTotal: 1,
      scrollResidualFullRepaintsTotal: 1,
      scrollZeroSavedBatchesTotal: 3,
      hostSentHashEntries: 0,
      hostSentHashEvictionsTotal: 0,
      hostCacheMissReportsTotal: 0,
    });

    const [zstdFrames] = parseFrames(wireFixture('tile_zstd'));
    const zstd = parseTileMessage(zstdFrames[0].payload);
    expect(zstd).toEqual({
      type: 'zstd',
      col: 2,
      row: 5,
      hash: 0x1122334455667788n,
      data: new Uint8Array([1, 2, 3, 4, 5]),
    });
  });

  it('returns null for the shared unknown tile tag fixture', () => {
    const [frames] = parseFrames(wireFixture('invalid_tile_unknown_tag'));
    expect(parseTileMessage(frames[0].payload)).toBeNull();
  });
});

// ── CH_TILES constant ───────────────────────────────────────────────

describe('CH_TILES', () => {
  it('equals 0x0B', () => {
    expect(CH_TILES).toBe(0x0B);
  });
});
