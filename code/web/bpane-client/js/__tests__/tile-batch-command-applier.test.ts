import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { TileCommand } from '../tile-cache.js';
import {
  TileBatchCommandApplier,
  type TileBatchCommandApplierCallbacks,
} from '../render/tile-batch-command-applier.js';

describe('TileBatchCommandApplier', () => {
  let callbacks: TileBatchCommandApplierCallbacks;
  let applier: TileBatchCommandApplier;

  beforeEach(() => {
    callbacks = {
      applyScrollCopy: vi.fn(),
      setGridOffset: vi.fn(),
      setApplyOffsetMode: vi.fn(),
      setVideoRegion: vi.fn(),
      drawFill: vi.fn(),
      drawCacheHit: vi.fn(),
      drawQoi: vi.fn(),
      drawZstd: vi.fn(),
    };
    applier = new TileBatchCommandApplier(callbacks);
  });

  it('routes state transitions and draw commands in batch order', () => {
    const events: string[] = [];
    vi.mocked(callbacks.setGridOffset).mockImplementation(() => events.push('grid-offset'));
    vi.mocked(callbacks.setApplyOffsetMode).mockImplementation(() => events.push('tile-draw-mode'));
    vi.mocked(callbacks.drawFill).mockImplementation(() => events.push('fill'));
    vi.mocked(callbacks.drawCacheHit).mockImplementation(() => events.push('cache-hit'));
    vi.mocked(callbacks.drawQoi).mockImplementation(() => events.push('qoi'));
    vi.mocked(callbacks.drawZstd).mockImplementation(() => events.push('zstd'));
    vi.mocked(callbacks.setVideoRegion).mockImplementation(() => events.push('video-region'));

    const qoiData = new Uint8Array([1, 2, 3]);
    const zstdData = new Uint8Array([4, 5, 6]);
    const commands: TileCommand[] = [
      { type: 'grid-offset', offsetX: 11, offsetY: -7 },
      { type: 'tile-draw-mode', applyOffset: false },
      { type: 'fill', col: 1, row: 2, rgba: 0xff332211 },
      { type: 'cache-hit', col: 3, row: 4, hash: 99n },
      { type: 'qoi', col: 5, row: 6, hash: 100n, data: qoiData },
      { type: 'zstd', col: 7, row: 8, hash: 101n, data: zstdData },
      { type: 'video-region', x: 10, y: 20, w: 30, h: 40 },
    ];

    expect(applier.applyCommands({ commands, frameSeq: 42, epoch: 7 })).toBe(true);

    expect(events).toEqual([
      'grid-offset',
      'tile-draw-mode',
      'fill',
      'cache-hit',
      'qoi',
      'zstd',
      'video-region',
    ]);
    expect(callbacks.setGridOffset).toHaveBeenCalledWith(11, -7);
    expect(callbacks.setApplyOffsetMode).toHaveBeenCalledWith(false);
    expect(callbacks.drawFill).toHaveBeenCalledWith(1, 2, 0xff332211);
    expect(callbacks.drawCacheHit).toHaveBeenCalledWith(3, 4, 99n, 42);
    expect(callbacks.drawQoi).toHaveBeenCalledWith(5, 6, 100n, qoiData, 7);
    expect(callbacks.drawZstd).toHaveBeenCalledWith(7, 8, 101n, zstdData, 7);
    expect(callbacks.setVideoRegion).toHaveBeenCalledWith({ x: 10, y: 20, w: 30, h: 40 });
  });

  it('clears video region when the command is zero-sized', () => {
    const commands: TileCommand[] = [
      { type: 'video-region', x: 10, y: 20, w: 0, h: 40 },
    ];

    expect(applier.applyCommands({ commands, frameSeq: 1, epoch: 2 })).toBe(true);

    expect(callbacks.setVideoRegion).toHaveBeenCalledWith(null);
  });

  it('ignores protocol control commands inside a queued batch', () => {
    const commands: TileCommand[] = [
      {
        type: 'grid-config',
        config: { tileSize: 64, cols: 10, rows: 10, screenW: 640, screenH: 640 },
      },
      { type: 'batch-end', frameSeq: 9 },
      {
        type: 'scroll-stats',
        scrollBatchesTotal: 1,
        scrollFullFallbacksTotal: 2,
        scrollPotentialTilesTotal: 3,
        scrollSavedTilesTotal: 4,
      },
    ];

    expect(applier.applyCommands({ commands, frameSeq: 1, epoch: 2 })).toBe(true);

    expect(callbacks.applyScrollCopy).not.toHaveBeenCalled();
    expect(callbacks.setGridOffset).not.toHaveBeenCalled();
    expect(callbacks.setApplyOffsetMode).not.toHaveBeenCalled();
    expect(callbacks.setVideoRegion).not.toHaveBeenCalled();
    expect(callbacks.drawFill).not.toHaveBeenCalled();
    expect(callbacks.drawCacheHit).not.toHaveBeenCalled();
    expect(callbacks.drawQoi).not.toHaveBeenCalled();
    expect(callbacks.drawZstd).not.toHaveBeenCalled();
  });

  it('stops applying later commands when the batch becomes stale', () => {
    const commands: TileCommand[] = [
      { type: 'fill', col: 0, row: 0, rgba: 0xff },
      { type: 'cache-hit', col: 1, row: 1, hash: 12n },
      { type: 'video-region', x: 10, y: 20, w: 30, h: 40 },
    ];
    let shouldContinue = true;
    vi.mocked(callbacks.drawFill).mockImplementation(() => {
      shouldContinue = false;
    });

    expect(applier.applyCommands({
      commands,
      frameSeq: 8,
      epoch: 4,
      shouldContinue: () => shouldContinue,
    })).toBe(false);

    expect(callbacks.drawFill).toHaveBeenCalledWith(0, 0, 0xff);
    expect(callbacks.drawCacheHit).not.toHaveBeenCalled();
    expect(callbacks.setVideoRegion).not.toHaveBeenCalled();
  });
});
