import { describe, expect, it } from 'vitest';

import { resolveTileRect, resolveTileRectInto } from '../render/tile-rect-resolver.js';
import type { TileGridConfig } from '../tile-cache.js';

const GRID_CONFIG: TileGridConfig = {
  tileSize: 64,
  cols: 10,
  rows: 10,
  screenW: 640,
  screenH: 640,
};

describe('resolveTileRect', () => {
  it('returns null when grid config is missing', () => {
    expect(resolveTileRect({
      gridConfig: null,
      col: 0,
      row: 0,
      gridOffsetX: 0,
      gridOffsetY: 0,
      applyOffset: true,
    })).toBeNull();
  });

  it('computes a tile rect with no offset', () => {
    expect(resolveTileRect({
      gridConfig: GRID_CONFIG,
      col: 3,
      row: 5,
      gridOffsetX: 0,
      gridOffsetY: 0,
      applyOffset: true,
    })).toEqual({
      x: 192,
      y: 320,
      w: 64,
      h: 64,
    });
  });

  it('applies grid offsets when offset mode is enabled', () => {
    expect(resolveTileRect({
      gridConfig: GRID_CONFIG,
      col: 1,
      row: 2,
      gridOffsetX: 0,
      gridOffsetY: 37,
      applyOffset: true,
    })).toEqual({
      x: 64,
      y: 91,
      w: 64,
      h: 64,
    });
  });

  it('ignores grid offsets when offset mode is disabled', () => {
    expect(resolveTileRect({
      gridConfig: GRID_CONFIG,
      col: 1,
      row: 2,
      gridOffsetX: 0,
      gridOffsetY: 37,
      applyOffset: false,
    })).toEqual({
      x: 64,
      y: 128,
      w: 64,
      h: 64,
    });
  });

  it('clips partially visible tiles to the screen bounds', () => {
    expect(resolveTileRect({
      gridConfig: GRID_CONFIG,
      col: 0,
      row: 0,
      gridOffsetX: 0,
      gridOffsetY: 37,
      applyOffset: true,
    })).toEqual({
      x: 0,
      y: 0,
      w: 64,
      h: 27,
    });

    expect(resolveTileRect({
      gridConfig: {
        ...GRID_CONFIG,
        screenW: 100,
        screenH: 100,
        cols: 2,
        rows: 2,
      },
      col: 1,
      row: 1,
      gridOffsetX: 0,
      gridOffsetY: 0,
      applyOffset: true,
    })).toEqual({
      x: 64,
      y: 64,
      w: 36,
      h: 36,
    });
  });

  it('returns null when the tile is fully clipped out of view', () => {
    expect(resolveTileRect({
      gridConfig: GRID_CONFIG,
      col: 0,
      row: 0,
      gridOffsetX: 0,
      gridOffsetY: 80,
      applyOffset: true,
    })).toBeNull();
  });

  it('writes into the provided output rect for hot-path callers', () => {
    const outRect = { x: -1, y: -1, w: -1, h: -1 };

    expect(resolveTileRectInto({
      gridConfig: GRID_CONFIG,
      col: 1,
      row: 2,
      gridOffsetX: 0,
      gridOffsetY: 37,
      applyOffset: true,
    }, outRect)).toBe(outRect);
    expect(outRect).toEqual({
      x: 64,
      y: 91,
      w: 64,
      h: 64,
    });
  });
});
