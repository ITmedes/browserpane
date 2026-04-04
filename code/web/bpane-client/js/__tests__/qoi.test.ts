import { describe, it, expect } from 'vitest';
import { decodeQoi } from '../qoi.js';

describe('decodeQoi', () => {
  it('decodes a 1x1 RGBA tile', () => {
    const qoi = new Uint8Array([
      0x71, 0x6f, 0x69, 0x66, // "qoif"
      0x00, 0x00, 0x00, 0x01, // width = 1
      0x00, 0x00, 0x00, 0x01, // height = 1
      0x04, // channels = RGBA
      0x00, // colorspace = sRGB with linear alpha
      0xff, 0xff, 0x00, 0x00, 0xff, // RGBA pixel: red
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // end marker
    ]);

    const decoded = decodeQoi(qoi);
    expect(decoded).not.toBeNull();
    expect(decoded!.width).toBe(1);
    expect(decoded!.height).toBe(1);
    expect(Array.from(decoded!.pixels)).toEqual([0xff, 0x00, 0x00, 0xff]);
  });

  it('decodes run chunks', () => {
    const qoi = new Uint8Array([
      0x71, 0x6f, 0x69, 0x66,
      0x00, 0x00, 0x00, 0x02, // width = 2
      0x00, 0x00, 0x00, 0x01, // height = 1
      0x04,
      0x00,
      0xfe, 0x10, 0x20, 0x30, // RGB chunk, alpha stays 255
      0xc0, // RUN (repeat previous pixel once)
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    ]);

    const decoded = decodeQoi(qoi);
    expect(decoded).not.toBeNull();
    expect(decoded!.width).toBe(2);
    expect(decoded!.height).toBe(1);
    expect(Array.from(decoded!.pixels)).toEqual([
      0x10, 0x20, 0x30, 0xff,
      0x10, 0x20, 0x30, 0xff,
    ]);
  });

  it('returns null for invalid payload', () => {
    expect(decodeQoi(new Uint8Array([0x00, 0x01, 0x02]))).toBeNull();
  });
});
