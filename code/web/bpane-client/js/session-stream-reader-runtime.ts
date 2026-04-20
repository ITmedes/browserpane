import { FRAME_HEADER_SIZE, parseFramesInto } from './protocol.js';

export interface SessionStreamReaderRuntimeInput {
  isConnected: () => boolean;
  recordRx: (channelId: number, bytes: number) => void;
  onFrame: (channelId: number, payload: Uint8Array) => void;
  onReadError?: (error: unknown) => void;
}

export class SessionStreamReaderRuntime {
  private readonly isConnected: () => boolean;
  private readonly recordRx: (channelId: number, bytes: number) => void;
  private readonly onFrame: (channelId: number, payload: Uint8Array) => void;
  private readonly onReadError?: (error: unknown) => void;

  constructor(input: SessionStreamReaderRuntimeInput) {
    this.isConnected = input.isConnected;
    this.recordRx = input.recordRx;
    this.onFrame = input.onFrame;
    this.onReadError = input.onReadError;
  }

  async readStream(stream: WebTransportBidirectionalStream): Promise<void> {
    const reader = stream.readable.getReader();
    let buffer = new Uint8Array(128 * 1024);
    let bufferLength = 0;

    try {
      while (this.isConnected()) {
        const { value, done } = await reader.read();
        if (done) {
          break;
        }
        if (!value) {
          continue;
        }

        const chunk = this.toUint8Array(value);
        const needed = bufferLength + chunk.length;
        if (needed > buffer.length) {
          const nextBuffer = new Uint8Array(Math.max(needed, buffer.length * 2));
          nextBuffer.set(buffer.subarray(0, bufferLength));
          buffer = nextBuffer;
        }

        buffer.set(chunk, bufferLength);
        bufferLength += chunk.length;

        const remaining = parseFramesInto(buffer.subarray(0, bufferLength), (channelId, payload) => {
          this.recordRx(channelId, payload.length + FRAME_HEADER_SIZE);
          this.onFrame(channelId, payload);
        });

        if (remaining.length > 0) {
          buffer.copyWithin(0, bufferLength - remaining.length, bufferLength);
        }
        bufferLength = remaining.length;
      }
    } catch (error) {
      if (!this.isConnected()) {
        return;
      }
      this.onReadError?.(error);
    }
  }

  private toUint8Array(value: ArrayBufferLike | ArrayBufferView): Uint8Array {
    if (value instanceof Uint8Array) {
      return value;
    }
    if (ArrayBuffer.isView(value)) {
      return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
    }
    return new Uint8Array(value);
  }
}
