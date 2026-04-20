import { describe, expect, it } from 'vitest';
import { parseFrames } from '../protocol.js';
import { CH_TILES, parseTileMessage } from '../render/tile-message-parser.js';
import { wireFixture } from './wire-fixtures.js';

function buildGridOffset(offsetX: number, offsetY: number): Uint8Array {
  const payload = new Uint8Array(5);
  const view = new DataView(payload.buffer);
  payload[0] = 0x08;
  view.setInt16(1, offsetX, true);
  view.setInt16(3, offsetY, true);
  return payload;
}

function buildTileDrawMode(flag: number): Uint8Array {
  return new Uint8Array([0x0B, flag]);
}

describe('tile-message-parser', () => {
  it('keeps the shared channel id', () => {
    expect(CH_TILES).toBe(0x0B);
  });

  it('parses zstd payloads from the shared wire fixtures', () => {
    const [frames] = parseFrames(wireFixture('tile_zstd'));
    const command = parseTileMessage(frames[0].payload);

    expect(command).toEqual({
      type: 'zstd',
      col: 2,
      row: 5,
      hash: 0x1122334455667788n,
      data: new Uint8Array([1, 2, 3, 4, 5]),
    });
  });

  it('parses signed grid offsets', () => {
    const command = parseTileMessage(buildGridOffset(-12, 34));

    expect(command).toEqual({
      type: 'grid-offset',
      offsetX: -12,
      offsetY: 34,
    });
  });

  it('treats non-zero tile draw mode bytes as true', () => {
    const command = parseTileMessage(buildTileDrawMode(7));

    expect(command).toEqual({
      type: 'tile-draw-mode',
      applyOffset: true,
    });
  });

  it('returns null for truncated zstd payloads', () => {
    const payload = new Uint8Array(19);
    const view = new DataView(payload.buffer);
    payload[0] = 0x0C;
    view.setUint32(13, 100, true);

    expect(parseTileMessage(payload)).toBeNull();
  });
});
