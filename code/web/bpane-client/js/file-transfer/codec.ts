import { ValidationError } from '../shared/errors.js';

const FILE_HEADER = 0x01;
const FILE_CHUNK = 0x02;
const FILE_COMPLETE = 0x03;
const FILE_NAME_BYTES = 256;
const FILE_MIME_BYTES = 64;

interface FileHeaderMessage {
  id: number;
  filename: string;
  size: number;
  mime: string;
}

interface FileChunkMessage {
  id: number;
  seq: number;
  data: Uint8Array;
}

export type DecodedFileMessage =
  | { type: 'header'; id: number; filename: string; size: number; mime: string }
  | { type: 'chunk'; id: number; seq: number; data: Uint8Array }
  | { type: 'complete'; id: number };

export class FileTransferCodec {
  static encodeHeader(message: FileHeaderMessage): Uint8Array {
    const payload = new Uint8Array(1 + 4 + FILE_NAME_BYTES + 8 + FILE_MIME_BYTES);
    const view = new DataView(payload.buffer);
    payload[0] = FILE_HEADER;
    view.setUint32(1, message.id >>> 0, true);
    payload.set(FileTransferCodec.encodeFixedString(message.filename, FILE_NAME_BYTES), 5);
    view.setBigUint64(5 + FILE_NAME_BYTES, BigInt(message.size), true);
    payload.set(FileTransferCodec.encodeFixedString(message.mime, FILE_MIME_BYTES), 13 + FILE_NAME_BYTES);
    return payload;
  }

  static encodeChunk(message: FileChunkMessage): Uint8Array {
    const payload = new Uint8Array(1 + 4 + 4 + 4 + message.data.byteLength);
    const view = new DataView(payload.buffer);
    payload[0] = FILE_CHUNK;
    view.setUint32(1, message.id >>> 0, true);
    view.setUint32(5, message.seq >>> 0, true);
    view.setUint32(9, message.data.byteLength >>> 0, true);
    payload.set(message.data, 13);
    return payload;
  }

  static encodeComplete(id: number): Uint8Array {
    const payload = new Uint8Array(1 + 4);
    const view = new DataView(payload.buffer);
    payload[0] = FILE_COMPLETE;
    view.setUint32(1, id >>> 0, true);
    return payload;
  }

  static decode(payload: Uint8Array): DecodedFileMessage {
    if (payload.byteLength < 1) {
      throw new ValidationError(
        'bpane.file_transfer.payload_too_short',
        'file payload too short',
      );
    }

    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    const tag = payload[0];
    if (tag === FILE_HEADER) {
      if (payload.byteLength < 1 + 4 + FILE_NAME_BYTES + 8 + FILE_MIME_BYTES) {
        throw new ValidationError(
          'bpane.file_transfer.header_too_short',
          'file header too short',
        );
      }
      return {
        type: 'header',
        id: view.getUint32(1, true),
        filename: FileTransferCodec.decodeFixedString(payload.subarray(5, 5 + FILE_NAME_BYTES)),
        size: Number(view.getBigUint64(5 + FILE_NAME_BYTES, true)),
        mime: FileTransferCodec.decodeFixedString(
          payload.subarray(13 + FILE_NAME_BYTES, 13 + FILE_NAME_BYTES + FILE_MIME_BYTES),
        ),
      };
    }
    if (tag === FILE_CHUNK) {
      if (payload.byteLength < 13) {
        throw new ValidationError(
          'bpane.file_transfer.chunk_too_short',
          'file chunk too short',
        );
      }
      const length = view.getUint32(9, true);
      if (payload.byteLength < 13 + length) {
        throw new ValidationError(
          'bpane.file_transfer.chunk_truncated',
          'file chunk truncated',
        );
      }
      return {
        type: 'chunk',
        id: view.getUint32(1, true),
        seq: view.getUint32(5, true),
        data: payload.subarray(13, 13 + length),
      };
    }
    if (tag === FILE_COMPLETE) {
      if (payload.byteLength < 5) {
        throw new ValidationError(
          'bpane.file_transfer.complete_too_short',
          'file completion too short',
        );
      }
      return {
        type: 'complete',
        id: view.getUint32(1, true),
      };
    }

    throw new ValidationError(
      'bpane.file_transfer.unknown_tag',
      `unknown file tag: ${tag}`,
    );
  }

  private static encodeFixedString(input: string, maxBytes: number): Uint8Array {
    const output = new Uint8Array(maxBytes);
    const encoder = new TextEncoder();
    let offset = 0;
    for (const char of input) {
      const encoded = encoder.encode(char);
      if (offset + encoded.byteLength > maxBytes) {
        break;
      }
      output.set(encoded, offset);
      offset += encoded.byteLength;
    }
    return output;
  }

  private static decodeFixedString(bytes: Uint8Array): string {
    let end = bytes.indexOf(0);
    if (end < 0) end = bytes.byteLength;
    return new TextDecoder().decode(bytes.subarray(0, end)).trim();
  }
}
