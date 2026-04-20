import { describe, expect, it } from 'vitest';

import { AUDIO_FRAME_HEADER_SIZE } from '../protocol.js';
import {
  AUDIO_CODEC_ADPCM_IMA_STEREO,
  AUDIO_CODEC_OPUS,
  AUDIO_CODEC_PCM_S16LE,
  decodeAudioFramePayload,
} from '../audio-frame-decoder.js';

const AUDIO_PAYLOAD_MAGIC = new Uint8Array([0x57, 0x52, 0x41, 0x31]);

function concatBytes(...chunks: Uint8Array[]): Uint8Array {
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function makeTransportFrame(rawPayload: Uint8Array): Uint8Array {
  const payload = new Uint8Array(AUDIO_FRAME_HEADER_SIZE + rawPayload.length);
  new DataView(payload.buffer).setUint32(12, rawPayload.length, true);
  payload.set(rawPayload, AUDIO_FRAME_HEADER_SIZE);
  return payload;
}

function makePcmBytes(samples: number[]): Uint8Array {
  const pcm = new Int16Array(samples);
  return new Uint8Array(pcm.buffer.slice(0));
}

describe('decodeAudioFramePayload', () => {
  it('decodes codec-tagged PCM payloads into float samples', () => {
    const result = decodeAudioFramePayload(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_PCM_S16LE]),
      makePcmBytes([32767, -32768, 16384, -16384]),
    )));

    expect(result).toEqual({
      kind: 'samples',
      samples: new Float32Array([
        32767 / 32768,
        -1,
        0.5,
        -0.5,
      ]),
    });
  });

  it('decodes legacy PCM payloads without a codec header', () => {
    const result = decodeAudioFramePayload(makeTransportFrame(makePcmBytes([8192, -8192])));

    expect(result).toEqual({
      kind: 'samples',
      samples: new Float32Array([0.25, -0.25]),
    });
  });

  it('decodes ADPCM stereo payloads', () => {
    const result = decodeAudioFramePayload(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_ADPCM_IMA_STEREO]),
      new Uint8Array([0, 0, 0, 0, 0, 0, 0]),
    )));

    expect(result).toEqual({
      kind: 'samples',
      samples: new Float32Array([0, 0, 0, 0]),
    });
  });

  it('returns Opus payloads without decoding them', () => {
    const result = decodeAudioFramePayload(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_OPUS, 1, 2, 3]),
    )));

    expect(result).toEqual({
      kind: 'opus',
      encoded: new Uint8Array([1, 2, 3]),
    });
  });

  it('returns null for truncated transport frames', () => {
    const payload = makeTransportFrame(new Uint8Array([1, 2, 3]));
    new DataView(payload.buffer).setUint32(12, 99, true);

    expect(decodeAudioFramePayload(payload)).toBeNull();
    expect(decodeAudioFramePayload(new Uint8Array(0))).toBeNull();
    expect(decodeAudioFramePayload(new Uint8Array(AUDIO_FRAME_HEADER_SIZE - 1))).toBeNull();
  });

  it('returns null for unknown codec tags', () => {
    const result = decodeAudioFramePayload(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([0xff, 1, 2, 3]),
    )));

    expect(result).toBeNull();
  });

  it('returns null for malformed ADPCM payloads', () => {
    const result = decodeAudioFramePayload(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_ADPCM_IMA_STEREO]),
      new Uint8Array([1, 2, 3, 4, 5]),
    )));

    expect(result).toBeNull();
  });
});
