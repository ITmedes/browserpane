import type { TileCommand } from '../tile-cache.js';

export const CH_TILES = 0x0B;

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

export class TileMessageParser {
  static parse(payload: Uint8Array): TileCommand | null {
    if (payload.length < 1) {
      return null;
    }

    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);

    switch (payload[0]) {
      case TILE_GRID_CONFIG:
        return TileMessageParser.parseGridConfig(payload, view);
      case TILE_CACHE_HIT:
        return TileMessageParser.parseCacheHit(payload, view);
      case TILE_FILL:
        return TileMessageParser.parseFill(payload, view);
      case TILE_QOI:
        return TileMessageParser.parseQoi(payload, view);
      case TILE_ZSTD:
        return TileMessageParser.parseZstd(payload, view);
      case TILE_VIDEO_REGION:
        return TileMessageParser.parseVideoRegion(payload, view);
      case TILE_BATCH_END:
        return TileMessageParser.parseBatchEnd(payload, view);
      case TILE_SCROLL_COPY:
        return TileMessageParser.parseScrollCopy(payload, view);
      case TILE_DRAW_MODE:
        return TileMessageParser.parseTileDrawMode(payload);
      case TILE_GRID_OFFSET:
        return TileMessageParser.parseGridOffset(payload, view);
      case TILE_SCROLL_STATS:
        return TileMessageParser.parseScrollStats(payload, view);
      default:
        return null;
    }
  }

  private static parseGridConfig(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 11) {
      return null;
    }

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

  private static parseCacheHit(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 13) {
      return null;
    }

    return {
      type: 'cache-hit',
      col: view.getUint16(1, true),
      row: view.getUint16(3, true),
      hash: view.getBigUint64(5, true),
    };
  }

  private static parseFill(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 9) {
      return null;
    }

    return {
      type: 'fill',
      col: view.getUint16(1, true),
      row: view.getUint16(3, true),
      rgba: view.getUint32(5, true),
    };
  }

  private static parseQoi(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 17) {
      return null;
    }

    const dataLength = view.getUint32(13, true);
    if (payload.length < 17 + dataLength) {
      return null;
    }

    return {
      type: 'qoi',
      col: view.getUint16(1, true),
      row: view.getUint16(3, true),
      hash: view.getBigUint64(5, true),
      data: payload.slice(17, 17 + dataLength),
    };
  }

  private static parseZstd(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 17) {
      return null;
    }

    const dataLength = view.getUint32(13, true);
    if (payload.length < 17 + dataLength) {
      return null;
    }

    return {
      type: 'zstd',
      col: view.getUint16(1, true),
      row: view.getUint16(3, true),
      hash: view.getBigUint64(5, true),
      data: payload.slice(17, 17 + dataLength),
    };
  }

  private static parseVideoRegion(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 9) {
      return null;
    }

    return {
      type: 'video-region',
      x: view.getUint16(1, true),
      y: view.getUint16(3, true),
      w: view.getUint16(5, true),
      h: view.getUint16(7, true),
    };
  }

  private static parseBatchEnd(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 5) {
      return null;
    }

    return {
      type: 'batch-end',
      frameSeq: view.getUint32(1, true),
    };
  }

  private static parseScrollCopy(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 11) {
      return null;
    }

    return {
      type: 'scroll-copy',
      dx: view.getInt16(1, true),
      dy: view.getInt16(3, true),
      regionTop: view.getUint16(5, true),
      regionBottom: view.getUint16(7, true),
      regionRight: view.getUint16(9, true),
    };
  }

  private static parseTileDrawMode(payload: Uint8Array): TileCommand | null {
    if (payload.length < 2) {
      return null;
    }

    return {
      type: 'tile-draw-mode',
      applyOffset: payload[1] !== 0,
    };
  }

  private static parseGridOffset(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 5) {
      return null;
    }

    return {
      type: 'grid-offset',
      offsetX: view.getInt16(1, true),
      offsetY: view.getInt16(3, true),
    };
  }

  private static parseScrollStats(payload: Uint8Array, view: DataView): TileCommand | null {
    if (payload.length < 29) {
      return null;
    }

    return {
      type: 'scroll-stats',
      scrollBatchesTotal: view.getUint32(1, true),
      scrollFullFallbacksTotal: view.getUint32(5, true),
      scrollPotentialTilesTotal: view.getUint32(9, true),
      scrollSavedTilesTotal: view.getUint32(13, true),
      scrollNonQuantizedFallbacksTotal: view.getUint32(17, true),
      scrollResidualFullRepaintsTotal: view.getUint32(21, true),
      scrollZeroSavedBatchesTotal: view.getUint32(25, true),
    };
  }
}

export function parseTileMessage(payload: Uint8Array): TileCommand | null {
  return TileMessageParser.parse(payload);
}
