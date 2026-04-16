import { AUDIO_FRAME_HEADER_SIZE } from './protocol.js';

const AUDIO_PAYLOAD_MAGIC0 = 0x57; // 'W'
const AUDIO_PAYLOAD_MAGIC1 = 0x52; // 'R'
const AUDIO_PAYLOAD_MAGIC2 = 0x41; // 'A'
const AUDIO_PAYLOAD_MAGIC3 = 0x31; // '1'

export const AUDIO_CODEC_PCM_S16LE = 0x00;
export const AUDIO_CODEC_ADPCM_IMA_STEREO = 0x01;
export const AUDIO_CODEC_OPUS = 0x02;

const IMA_INDEX_TABLE = new Int8Array([
  -1, -1, -1, -1, 2, 4, 6, 8,
  -1, -1, -1, -1, 2, 4, 6, 8,
]);

const IMA_STEP_TABLE = new Int16Array([
  7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31,
  34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130,
  143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
  494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411,
  1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026,
  4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442,
  11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623,
  27086, 29794, 32767,
]);

export type DecodedAudioFrame =
  | { kind: 'samples'; samples: Float32Array }
  | { kind: 'opus'; encoded: Uint8Array };

export function decodeAudioFramePayload(payload: Uint8Array): DecodedAudioFrame | null {
  if (payload.length < AUDIO_FRAME_HEADER_SIZE) {
    return null;
  }

  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  const dataLen = view.getUint32(12, true);
  if (payload.length < AUDIO_FRAME_HEADER_SIZE + dataLen) {
    return null;
  }

  const raw = payload.subarray(AUDIO_FRAME_HEADER_SIZE, AUDIO_FRAME_HEADER_SIZE + dataLen);
  const hasMagic = raw.length >= 5
    && raw[0] === AUDIO_PAYLOAD_MAGIC0
    && raw[1] === AUDIO_PAYLOAD_MAGIC1
    && raw[2] === AUDIO_PAYLOAD_MAGIC2
    && raw[3] === AUDIO_PAYLOAD_MAGIC3;

  if (!hasMagic) {
    const samples = decodePcmS16le(raw);
    return samples.length > 0 ? { kind: 'samples', samples } : null;
  }

  const codec = raw[4];
  const encoded = raw.subarray(5);
  if (codec === AUDIO_CODEC_OPUS) {
    return { kind: 'opus', encoded };
  }
  if (codec === AUDIO_CODEC_PCM_S16LE) {
    const samples = decodePcmS16le(encoded);
    return samples.length > 0 ? { kind: 'samples', samples } : null;
  }
  if (codec === AUDIO_CODEC_ADPCM_IMA_STEREO) {
    const samples = decodeAdpcmImaStereo(encoded);
    return samples && samples.length > 0 ? { kind: 'samples', samples } : null;
  }

  return null;
}

function decodePcmS16le(data: Uint8Array): Float32Array {
  const sampleCount = Math.floor(data.length / 2);
  const view = new DataView(data.buffer, data.byteOffset, sampleCount * 2);
  const floats = new Float32Array(sampleCount);
  for (let i = 0; i < sampleCount; i++) {
    floats[i] = view.getInt16(i * 2, true) / 32768.0;
  }
  return floats;
}

function decodeAdpcmImaStereo(data: Uint8Array): Float32Array | null {
  if (data.length < 6) {
    return null;
  }

  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  let leftPred = view.getInt16(0, true);
  let leftIndex = Math.max(0, Math.min(88, data[2]));
  let rightPred = view.getInt16(3, true);
  let rightIndex = Math.max(0, Math.min(88, data[5]));
  const packed = data.length - 6;
  const samplesPerChannel = packed + 1;
  const out = new Float32Array(samplesPerChannel * 2);
  let pos = 0;
  out[pos++] = leftPred / 32768.0;
  out[pos++] = rightPred / 32768.0;

  for (let i = 0; i < packed; i++) {
    const byte = data[6 + i];
    const leftNibble = byte & 0x0f;
    const rightNibble = (byte >> 4) & 0x0f;

    let step = IMA_STEP_TABLE[leftIndex];
    let diff = step >> 3;
    if (leftNibble & 0x01) diff += step >> 2;
    if (leftNibble & 0x02) diff += step >> 1;
    if (leftNibble & 0x04) diff += step;
    leftPred += (leftNibble & 0x08) ? -diff : diff;
    if (leftPred > 32767) leftPred = 32767;
    else if (leftPred < -32768) leftPred = -32768;
    leftIndex += IMA_INDEX_TABLE[leftNibble];
    if (leftIndex < 0) leftIndex = 0;
    else if (leftIndex > 88) leftIndex = 88;

    step = IMA_STEP_TABLE[rightIndex];
    diff = step >> 3;
    if (rightNibble & 0x01) diff += step >> 2;
    if (rightNibble & 0x02) diff += step >> 1;
    if (rightNibble & 0x04) diff += step;
    rightPred += (rightNibble & 0x08) ? -diff : diff;
    if (rightPred > 32767) rightPred = 32767;
    else if (rightPred < -32768) rightPred = -32768;
    rightIndex += IMA_INDEX_TABLE[rightNibble];
    if (rightIndex < 0) rightIndex = 0;
    else if (rightIndex > 88) rightIndex = 88;

    out[pos++] = leftPred / 32768.0;
    out[pos++] = rightPred / 32768.0;
  }

  return out;
}
