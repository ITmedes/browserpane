import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TileInfo } from '../nal.js';
import { SessionVideoDecoderRuntime } from '../session-video-decoder-runtime.js';

class MockVideoDecoder {
  static instances: MockVideoDecoder[] = [];

  readonly outputCallback: (frame: VideoFrame) => void;
  readonly errorCallback: (error: DOMException) => void;
  configured = false;
  closed = false;
  decodeQueueSize = 0;
  decodedChunks: MockEncodedVideoChunk[] = [];

  constructor(init: VideoDecoderInit) {
    this.outputCallback = init.output;
    this.errorCallback = init.error;
    MockVideoDecoder.instances.push(this);
  }

  configure(): void {
    this.configured = true;
  }

  decode(chunk: EncodedVideoChunk): void {
    const typedChunk = chunk as unknown as MockEncodedVideoChunk;
    this.decodedChunks.push(typedChunk);
    this.outputCallback({
      close: vi.fn(),
      timestamp: typedChunk.timestamp,
    } as unknown as VideoFrame);
  }

  close(): void {
    this.closed = true;
  }
}

class MockEncodedVideoChunk {
  type: EncodedVideoChunkType;
  timestamp: number;
  data: Uint8Array;

  constructor(init: EncodedVideoChunkInit) {
    this.type = init.type;
    this.timestamp = init.timestamp;
    this.data = ArrayBuffer.isView(init.data)
      ? Uint8Array.from(
        new Uint8Array(init.data.buffer, init.data.byteOffset, init.data.byteLength),
      )
      : Uint8Array.from(new Uint8Array(init.data));
  }
}

function createTileInfo(): TileInfo {
  return {
    tileX: 10,
    tileY: 20,
    tileW: 30,
    tileH: 40,
    screenW: 320,
    screenH: 180,
  };
}

describe('SessionVideoDecoderRuntime', () => {
  beforeEach(() => {
    MockVideoDecoder.instances = [];
  });

  it('caches SPS/PPS/SEI and prepends them to keyframes', () => {
    const onDecodedFrame = vi.fn();
    const incrementFrameCount = vi.fn();
    const runtime = new SessionVideoDecoderRuntime({
      onDecodedFrame,
      incrementFrameCount,
      incrementDroppedFrame: vi.fn(),
      onDecoderError: vi.fn(),
      videoDecoderCtor: MockVideoDecoder as unknown as typeof VideoDecoder,
      encodedVideoChunkCtor: MockEncodedVideoChunk as unknown as typeof EncodedVideoChunk,
    });
    const sps = new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42]);
    const pps = new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce]);
    const sei = new Uint8Array([0x00, 0x00, 0x01, 0x06, 0x05]);
    const idr = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88]);

    runtime.decodeNal(sps, null);
    runtime.decodeNal(pps, null);
    runtime.decodeNal(sei, null);
    runtime.decodeNal(idr, createTileInfo());

    expect(MockVideoDecoder.instances).toHaveLength(1);
    const decoder = MockVideoDecoder.instances[0];
    expect(decoder.configured).toBe(true);
    expect(decoder.decodedChunks).toHaveLength(1);
    expect(decoder.decodedChunks[0].type).toBe('key');
    expect(decoder.decodedChunks[0].timestamp).toBe(1);
    expect(decoder.decodedChunks[0].data).toEqual(new Uint8Array([
      ...sps,
      ...pps,
      ...sei,
      ...idr,
    ]));
    expect(incrementFrameCount).toHaveBeenCalledOnce();
    expect(onDecodedFrame).toHaveBeenCalledWith(expect.any(Object), createTileInfo());
  });

  it('ignores non-VCL NALs and does not create a decoder for SEI alone', () => {
    const runtime = new SessionVideoDecoderRuntime({
      onDecodedFrame: vi.fn(),
      incrementFrameCount: vi.fn(),
      incrementDroppedFrame: vi.fn(),
      onDecoderError: vi.fn(),
      videoDecoderCtor: MockVideoDecoder as unknown as typeof VideoDecoder,
      encodedVideoChunkCtor: MockEncodedVideoChunk as unknown as typeof EncodedVideoChunk,
    });

    runtime.decodeNal(new Uint8Array([0x00, 0x00, 0x01, 0x06, 0x05]), null);

    expect(MockVideoDecoder.instances).toHaveLength(0);
  });

  it('drops frames under decoder backpressure and records the drop', () => {
    const incrementDroppedFrame = vi.fn();
    const runtime = new SessionVideoDecoderRuntime({
      onDecodedFrame: vi.fn(),
      incrementFrameCount: vi.fn(),
      incrementDroppedFrame,
      onDecoderError: vi.fn(),
      videoDecoderCtor: MockVideoDecoder as unknown as typeof VideoDecoder,
      encodedVideoChunkCtor: MockEncodedVideoChunk as unknown as typeof EncodedVideoChunk,
    });
    const sps = new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42]);
    const pps = new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce]);
    const idr = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88]);

    runtime.decodeNal(sps, null);
    runtime.decodeNal(pps, null);
    runtime.ensureDecoder();
    MockVideoDecoder.instances[0].decodeQueueSize = 4;
    runtime.decodeNal(idr, null);

    expect(MockVideoDecoder.instances[0].decodedChunks).toHaveLength(0);
    expect(incrementDroppedFrame).toHaveBeenCalledOnce();
  });

  it('waits for the next keyframe after a decoder error before resuming', () => {
    const onDecoderError = vi.fn();
    const runtime = new SessionVideoDecoderRuntime({
      onDecodedFrame: vi.fn(),
      incrementFrameCount: vi.fn(),
      incrementDroppedFrame: vi.fn(),
      onDecoderError,
      videoDecoderCtor: MockVideoDecoder as unknown as typeof VideoDecoder,
      encodedVideoChunkCtor: MockEncodedVideoChunk as unknown as typeof EncodedVideoChunk,
    });
    const sps = new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42]);
    const pps = new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce]);
    const idr = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88]);
    const delta = new Uint8Array([0x00, 0x00, 0x01, 0x41, 0x99]);
    const secondIdr = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0xaa]);

    runtime.decodeNal(sps, null);
    runtime.decodeNal(pps, null);
    runtime.decodeNal(idr, null);

    MockVideoDecoder.instances[0].errorCallback(new DOMException('boom'));
    runtime.decodeNal(delta, null);
    runtime.decodeNal(secondIdr, null);

    expect(onDecoderError).toHaveBeenCalledOnce();
    expect(MockVideoDecoder.instances).toHaveLength(2);
    expect(MockVideoDecoder.instances[0].decodedChunks).toHaveLength(1);
    expect(MockVideoDecoder.instances[1].decodedChunks).toHaveLength(1);
  });

  it('routes decoded frame callbacks with the matching tile info timestamp', () => {
    const onDecodedFrame = vi.fn();
    const runtime = new SessionVideoDecoderRuntime({
      onDecodedFrame,
      incrementFrameCount: vi.fn(),
      incrementDroppedFrame: vi.fn(),
      onDecoderError: vi.fn(),
      videoDecoderCtor: MockVideoDecoder as unknown as typeof VideoDecoder,
      encodedVideoChunkCtor: MockEncodedVideoChunk as unknown as typeof EncodedVideoChunk,
    });
    const sps = new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42]);
    const pps = new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce]);
    const idr = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88]);
    const tileInfo = createTileInfo();

    runtime.decodeNal(sps, null);
    runtime.decodeNal(pps, null);
    runtime.decodeNal(idr, tileInfo);

    expect(onDecodedFrame).toHaveBeenCalledWith(expect.objectContaining({ timestamp: 1 }), tileInfo);
  });

  it('closes the active decoder and clears state on destroy', () => {
    const runtime = new SessionVideoDecoderRuntime({
      onDecodedFrame: vi.fn(),
      incrementFrameCount: vi.fn(),
      incrementDroppedFrame: vi.fn(),
      onDecoderError: vi.fn(),
      videoDecoderCtor: MockVideoDecoder as unknown as typeof VideoDecoder,
      encodedVideoChunkCtor: MockEncodedVideoChunk as unknown as typeof EncodedVideoChunk,
    });
    const sps = new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42]);
    const pps = new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce]);
    const idr = new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88]);

    runtime.decodeNal(sps, null);
    runtime.decodeNal(pps, null);
    runtime.decodeNal(idr, null);
    runtime.destroy();

    expect(MockVideoDecoder.instances[0].closed).toBe(true);
    expect(() => runtime.decodeNal(idr, null)).not.toThrow();
  });
});
