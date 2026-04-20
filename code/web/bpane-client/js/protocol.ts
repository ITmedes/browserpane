/**
 * BrowserPane wire protocol: frame encoding/decoding, channel IDs, constants.
 */

export const FRAME_HEADER_SIZE = 5; // channel(1) + length(4)
export const MAX_FRAME_PAYLOAD = 16 * 1024 * 1024; // 16 MiB sanity limit

// Channel IDs (must match bpane-protocol/src/channel.rs)
export const CH_VIDEO     = 0x01;
export const CH_AUDIO_OUT = 0x02;
export const CH_AUDIO_IN  = 0x03;
export const CH_VIDEO_IN  = 0x04;
export const CH_INPUT     = 0x05;
export const CH_CURSOR    = 0x06;
export const CH_CLIPBOARD = 0x07;
export const CH_FILE_UP   = 0x08;
export const CH_FILE_DOWN = 0x09;
export const CH_CONTROL   = 0x0A;

// Control message tags
export const CTRL_RESOLUTION_REQUEST = 0x01;
export const CTRL_RESOLUTION_ACK     = 0x02;
export const CTRL_SESSION_READY      = 0x03;
export const CTRL_PING               = 0x04;
export const CTRL_PONG               = 0x05;
export const CTRL_KEYBOARD_LAYOUT    = 0x06;
export const CTRL_BITRATE_HINT       = 0x07;

// Input message tags
export const INPUT_MOUSE_MOVE   = 0x01;
export const INPUT_MOUSE_BUTTON = 0x02;
export const INPUT_MOUSE_SCROLL = 0x03;
export const INPUT_KEY_EVENT    = 0x04;
export const INPUT_KEY_EVENT_EX = 0x05;

// Cursor message tags
export const CURSOR_MOVE  = 0x01;
export const CURSOR_SHAPE = 0x02;

// Clipboard message tags
export const CLIP_TEXT = 0x01;

// Audio frame header size: seq(4) + timestamp_us(8) + data_len(4) = 16
export const AUDIO_FRAME_HEADER_SIZE = 16;

/**
 * Encode a frame: channel(1) + payload_length(4 LE) + payload.
 */
export function encodeFrame(channelId: number, payload: Uint8Array): Uint8Array {
  const frame = new Uint8Array(FRAME_HEADER_SIZE + payload.length);
  frame[0] = channelId;
  frame[1] = payload.length & 0xFF;
  frame[2] = (payload.length >> 8) & 0xFF;
  frame[3] = (payload.length >> 16) & 0xFF;
  frame[4] = (payload.length >> 24) & 0xFF;
  frame.set(payload, FRAME_HEADER_SIZE);
  return frame;
}

/**
 * Parsed frame from the wire.
 */
export interface ParsedFrame {
  channelId: number;
  payload: Uint8Array;
}

export type FrameVisitor = (channelId: number, payload: Uint8Array) => void;

/**
 * Parse complete frames from a buffer and invoke `onFrame` for each one.
 * Returns the remaining incomplete suffix as a view into `buf`.
 */
export function parseFramesInto(buf: Uint8Array, onFrame: FrameVisitor): Uint8Array {
  let offset = 0;

  while (offset + FRAME_HEADER_SIZE <= buf.length) {
    const channelId = buf[offset];
    const length =
      (buf[offset + 1] |
      (buf[offset + 2] << 8) |
      (buf[offset + 3] << 16) |
      (buf[offset + 4] << 24)) >>> 0;

    if (length > MAX_FRAME_PAYLOAD) {
      throw new Error(`frame payload too large: ${length} bytes`);
    }

    const totalSize = FRAME_HEADER_SIZE + length;
    if (offset + totalSize > buf.length) break;

    onFrame(channelId, buf.subarray(offset + FRAME_HEADER_SIZE, offset + totalSize));
    offset += totalSize;
  }

  return buf.subarray(offset);
}

/**
 * Parse as many complete frames as possible from a buffer.
 * Returns [parsed_frames, remaining_bytes].
 * Throws on frames exceeding MAX_FRAME_PAYLOAD.
 */
export function parseFrames(buf: Uint8Array): [ParsedFrame[], Uint8Array] {
  const frames: ParsedFrame[] = [];
  const remaining = parseFramesInto(buf, (channelId, payload) => {
    frames.push({
      channelId,
      payload,
    });
  });
  return [frames, remaining];
}
