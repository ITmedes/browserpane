import { describe, expect, it } from 'vitest';

import { decodeAudioFramePayload } from '../audio-frame-decoder.js';
import { FileTransferCodec } from '../file-transfer/codec.js';
import { NalReassembler } from '../nal.js';
import {
  CH_AUDIO_OUT,
  CH_AUDIO_IN,
  CH_CLIPBOARD,
  CH_CONTROL,
  CH_CURSOR,
  CH_FILE_DOWN,
  CH_FILE_UP,
  CH_INPUT,
  CH_VIDEO,
  CH_VIDEO_IN,
  parseFrames,
} from '../protocol.js';
import { CH_TILES, parseTileMessage } from '../render/tile-message-parser.js';
import { SessionCursorRuntime } from '../session-cursor-runtime.js';
import {
  wireFixture,
  wireFixtureCatalog,
  type WireFixture,
} from './wire-fixtures.js';

const EXPECTED_FIXTURE_COUNT = 15;
const CHANNEL_IDS: Readonly<Record<string, number>> = {
  video: CH_VIDEO,
  audio_out: CH_AUDIO_OUT,
  audio_in: CH_AUDIO_IN,
  video_in: CH_VIDEO_IN,
  input: CH_INPUT,
  cursor: CH_CURSOR,
  clipboard: CH_CLIPBOARD,
  file_up: CH_FILE_UP,
  file_down: CH_FILE_DOWN,
  control: CH_CONTROL,
  tiles: CH_TILES,
};
const CAPABILITIES = new Set([
  'tile_zstd',
  'tile_cache',
  'tile_scroll',
  'h264_video',
  'roi_video',
  'audio_pcm_s16le',
  'audio_adpcm_ima_stereo',
  'audio_opus',
  'microphone_opus',
  'camera_h264_annex_b',
  'clipboard_text',
  'file_transfer',
  'extended_keyboard',
  'client_access_state',
]);

type Classification =
  | { outcome: 'valid'; message: string }
  | { outcome: 'invalid'; error: string };

describe('shared wire fixture catalog', () => {
  it('validates schema metadata and consumes every vector', () => {
    const catalog = wireFixtureCatalog();
    expect(catalog.schema_version).toBe(2);
    expect(catalog.catalog).toBe('browserpane-v1-conformance');
    expect(catalog.vectors).toHaveLength(EXPECTED_FIXTURE_COUNT);
    expect(new Set(catalog.vectors.map((fixture) => fixture.name)).size).toBe(
      EXPECTED_FIXTURE_COUNT,
    );

    for (const fixture of catalog.vectors) {
      expect(CHANNEL_IDS[fixture.channel], fixture.name).toBeDefined();
      expect(fixture.capabilities, fixture.name).toEqual(
        [...fixture.capabilities].sort(),
      );
      expect(new Set(fixture.capabilities).size, fixture.name).toBe(
        fixture.capabilities.length,
      );
      for (const capability of fixture.capabilities) {
        expect(CAPABILITIES.has(capability), `${fixture.name}: ${capability}`).toBe(true);
      }
      expect(classify(fixture), fixture.name).toEqual(fixture.expected);
    }
  });

  it('decodes the previously unconsumed audio and cursor vectors', () => {
    const [audioFrames] = parseFrames(wireFixture('audio_out_frame'));
    expect(decodeAudioFramePayload(audioFrames[0].payload)).toEqual({
      kind: 'opus',
      encoded: new Uint8Array([0x01, 0x02]),
    });

    const [cursorFrames] = parseFrames(wireFixture('cursor_shape_small'));
    const cursor = new SessionCursorRuntime({
      canvas: document.createElement('canvas'),
      cursorEl: null,
      cursorCtx: null,
    });
    expect(cursor.handlePayload(cursorFrames[0].payload)).toBe(true);
  });
});

function classify(fixture: WireFixture): Classification {
  const bytes = wireFixture(fixture.name);
  if (fixture.transport === 'datagram_payload') {
    const decoded = new NalReassembler().push(bytes);
    return decoded
      ? { outcome: 'valid', message: 'video_datagram' }
      : { outcome: 'invalid', error: 'buffer_too_short' };
  }

  let frames: ReturnType<typeof parseFrames>[0];
  let remaining: Uint8Array;
  try {
    [frames, remaining] = parseFrames(bytes);
  } catch (error) {
    if (error instanceof Error && error.message.includes('payload too large')) {
      return { outcome: 'invalid', error: 'payload_too_large' };
    }
    throw error;
  }
  if (remaining.length > 0 || frames.length !== 1) {
    return { outcome: 'invalid', error: 'buffer_too_short' };
  }

  const frame = frames[0];
  if (frame.channelId !== CHANNEL_IDS[fixture.channel]) {
    return { outcome: 'invalid', error: 'channel_metadata_mismatch' };
  }
  return classifyPayload(frame.channelId, frame.payload);
}

function classifyPayload(channelId: number, payload: Uint8Array): Classification {
  if (channelId === CH_AUDIO_OUT) {
    return decodeAudioFramePayload(payload)
      ? { outcome: 'valid', message: 'audio_frame' }
      : { outcome: 'invalid', error: 'buffer_too_short' };
  }
  if (channelId === CH_CURSOR) {
    const cursor = new SessionCursorRuntime({
      canvas: document.createElement('canvas'),
      cursorEl: null,
      cursorCtx: null,
    });
    return cursor.handlePayload(payload)
      ? { outcome: 'valid', message: 'cursor_shape' }
      : { outcome: 'invalid', error: 'buffer_too_short' };
  }
  if (channelId === CH_FILE_UP || channelId === CH_FILE_DOWN) {
    return classifyFile(payload);
  }
  if (channelId === CH_TILES) {
    return classifyTile(payload);
  }
  if (channelId === CH_CONTROL && payload.length === 3 && payload[0] === 0x03) {
    return { outcome: 'valid', message: 'control_session_ready' };
  }
  if (channelId === CH_INPUT && payload.length === 11 && payload[0] === 0x05) {
    return { outcome: 'valid', message: 'input_key_event_ex' };
  }
  if (channelId === CH_CLIPBOARD && hasExactLengthPrefixedBody(payload, 0x01)) {
    return { outcome: 'valid', message: 'clipboard_text' };
  }
  return { outcome: 'invalid', error: 'unknown_message_type' };
}

function classifyFile(payload: Uint8Array): Classification {
  try {
    const decoded = FileTransferCodec.decode(payload);
    return { outcome: 'valid', message: `file_${decoded.type}` };
  } catch (error) {
    if (error instanceof Error && /short|truncated/.test(error.message)) {
      return { outcome: 'invalid', error: 'buffer_too_short' };
    }
    return { outcome: 'invalid', error: 'unknown_message_type' };
  }
}

function classifyTile(payload: Uint8Array): Classification {
  const decoded = parseTileMessage(payload);
  if (!decoded) {
    return { outcome: 'invalid', error: 'unknown_message_type' };
  }
  return { outcome: 'valid', message: `tile_${decoded.type.replaceAll('-', '_')}` };
}

function hasExactLengthPrefixedBody(payload: Uint8Array, tag: number): boolean {
  if (payload.length < 5 || payload[0] !== tag) {
    return false;
  }
  const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
  return payload.length === 5 + view.getUint32(1, true);
}
