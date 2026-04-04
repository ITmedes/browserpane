/**
 * Minimal QOI decoder optimized for small tile images.
 * Returns RGBA pixels for immediate canvas compositing.
 */

export interface DecodedQoi {
  width: number;
  height: number;
  pixels: Uint8ClampedArray<ArrayBuffer>;
}

const QOI_OP_INDEX = 0x00;
const QOI_OP_DIFF = 0x40;
const QOI_OP_LUMA = 0x80;
const QOI_OP_RUN = 0xC0;
const QOI_OP_RGB = 0xFE;
const QOI_OP_RGBA = 0xFF;
const QOI_MASK_2 = 0xC0;

const QOI_HEADER_SIZE = 14;
const QOI_END_MARKER_SIZE = 8;
const QOI_END_MARKER = [0, 0, 0, 0, 0, 0, 0, 1];
const MAX_PIXELS = 4096 * 4096;

function readU32BE(data: Uint8Array, offset: number): number {
  return (
    (data[offset] << 24) |
    (data[offset + 1] << 16) |
    (data[offset + 2] << 8) |
    data[offset + 3]
  ) >>> 0;
}

function qoiHashIndex(r: number, g: number, b: number, a: number): number {
  return ((r * 3 + g * 5 + b * 7 + a * 11) & 63) * 4;
}

/**
 * Decode QOI bytes into RGBA pixels.
 * Returns null when the payload is malformed.
 */
export function decodeQoi(data: Uint8Array): DecodedQoi | null {
  if (data.length < QOI_HEADER_SIZE + QOI_END_MARKER_SIZE) return null;
  if (data[0] !== 0x71 || data[1] !== 0x6f || data[2] !== 0x69 || data[3] !== 0x66) {
    return null;
  }

  const width = readU32BE(data, 4);
  const height = readU32BE(data, 8);
  const channels = data[12];
  if (width === 0 || height === 0) return null;
  if (channels !== 3 && channels !== 4) return null;

  const pixelCount = width * height;
  if (!Number.isFinite(pixelCount) || pixelCount <= 0 || pixelCount > MAX_PIXELS) {
    return null;
  }

  const out = new Uint8ClampedArray(new ArrayBuffer(pixelCount * 4));
  const index = new Uint8Array(64 * 4);

  let p = QOI_HEADER_SIZE;
  let outPos = 0;
  let run = 0;

  let r = 0;
  let g = 0;
  let b = 0;
  let a = 255;

  for (let px = 0; px < pixelCount; px++) {
    if (run > 0) {
      run--;
    } else {
      if (p >= data.length) return null;
      const b1 = data[p++];

      if (b1 === QOI_OP_RGB) {
        if (p + 2 >= data.length) return null;
        r = data[p++];
        g = data[p++];
        b = data[p++];
      } else if (b1 === QOI_OP_RGBA) {
        if (p + 3 >= data.length) return null;
        r = data[p++];
        g = data[p++];
        b = data[p++];
        a = data[p++];
      } else {
        const tag = b1 & QOI_MASK_2;
        if (tag === QOI_OP_INDEX) {
          const idx = (b1 & 0x3F) * 4;
          r = index[idx];
          g = index[idx + 1];
          b = index[idx + 2];
          a = index[idx + 3];
        } else if (tag === QOI_OP_DIFF) {
          r = (r + ((b1 >> 4) & 0x03) - 2) & 0xFF;
          g = (g + ((b1 >> 2) & 0x03) - 2) & 0xFF;
          b = (b + (b1 & 0x03) - 2) & 0xFF;
        } else if (tag === QOI_OP_LUMA) {
          if (p >= data.length) return null;
          const b2 = data[p++];
          const vg = (b1 & 0x3F) - 32;
          const vr = ((b2 >> 4) & 0x0F) - 8;
          const vb = (b2 & 0x0F) - 8;
          r = (r + vg + vr) & 0xFF;
          g = (g + vg) & 0xFF;
          b = (b + vg + vb) & 0xFF;
        } else if (tag === QOI_OP_RUN) {
          run = b1 & 0x3F;
        } else {
          return null;
        }
      }
    }

    out[outPos++] = r;
    out[outPos++] = g;
    out[outPos++] = b;
    out[outPos++] = a;

    const h = qoiHashIndex(r, g, b, a);
    index[h] = r;
    index[h + 1] = g;
    index[h + 2] = b;
    index[h + 3] = a;
  }

  if (p + QOI_END_MARKER_SIZE > data.length) return null;
  for (let i = 0; i < QOI_END_MARKER_SIZE; i++) {
    if (data[p + i] !== QOI_END_MARKER[i]) return null;
  }

  return { width, height, pixels: out };
}
