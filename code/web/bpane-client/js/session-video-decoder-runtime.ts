import { getNalType, type TileInfo } from './nal.js';

export interface SessionVideoDecoderRuntimeInput {
  onDecodedFrame: (frame: VideoFrame, tileInfo: TileInfo | null) => void;
  incrementFrameCount: () => void;
  incrementDroppedFrame: () => void;
  onDecoderError: (error: DOMException) => void;
  videoDecoderCtor?: typeof VideoDecoder;
  encodedVideoChunkCtor?: typeof EncodedVideoChunk;
}

export class SessionVideoDecoderRuntime {
  private readonly onDecodedFrame: (frame: VideoFrame, tileInfo: TileInfo | null) => void;
  private readonly incrementFrameCount: () => void;
  private readonly incrementDroppedFrame: () => void;
  private readonly onDecoderError: (error: DOMException) => void;
  private readonly videoDecoderCtor: typeof VideoDecoder;
  private readonly encodedVideoChunkCtor: typeof EncodedVideoChunk;

  private videoDecoder: VideoDecoder | null = null;
  private decoderConfigured = false;
  private decoderTimestamp = 0;
  private spsNal: Uint8Array | null = null;
  private ppsNal: Uint8Array | null = null;
  private seiNals: Uint8Array[] = [];
  private readonly tileInfoByTimestamp = new Map<number, TileInfo | null>();

  constructor(input: SessionVideoDecoderRuntimeInput) {
    this.onDecodedFrame = input.onDecodedFrame;
    this.incrementFrameCount = input.incrementFrameCount;
    this.incrementDroppedFrame = input.incrementDroppedFrame;
    this.onDecoderError = input.onDecoderError;
    this.videoDecoderCtor = input.videoDecoderCtor ?? VideoDecoder;
    this.encodedVideoChunkCtor = input.encodedVideoChunkCtor ?? EncodedVideoChunk;
  }

  destroy(): void {
    this.closeDecoder();
    this.decoderConfigured = false;
    this.decoderTimestamp = 0;
    this.spsNal = null;
    this.ppsNal = null;
    this.seiNals = [];
    this.tileInfoByTimestamp.clear();
  }

  ensureDecoder(): void {
    if (!this.videoDecoder) {
      this.initDecoder();
    }
  }

  decodeNal(nalData: Uint8Array, tileInfo: TileInfo | null): void {
    const nalType = getNalType(nalData);

    if (nalType === 7) {
      this.spsNal = Uint8Array.from(nalData);
      return;
    }
    if (nalType === 8) {
      this.ppsNal = Uint8Array.from(nalData);
      return;
    }
    if (nalType === 6) {
      this.seiNals.push(Uint8Array.from(nalData));
      return;
    }
    if (nalType !== 1 && nalType !== 5) {
      return;
    }

    if (!this.videoDecoder) {
      this.initDecoder();
    }
    if (!this.decoderConfigured && nalType !== 5) {
      return;
    }
    if (!this.decoderConfigured) {
      this.initDecoder();
    }

    const decoder = this.videoDecoder!;
    if (decoder.decodeQueueSize > 3) {
      this.incrementDroppedFrame();
      return;
    }

    const chunkData = this.buildChunk(nalData, nalType);
    const timestamp = this.decoderTimestamp + 1;
    this.incrementFrameCount();
    this.tileInfoByTimestamp.set(timestamp, tileInfo);

    try {
      decoder.decode(new this.encodedVideoChunkCtor({
        type: nalType === 5 ? 'key' : 'delta',
        timestamp,
        data: chunkData,
      }));
      this.decoderTimestamp = timestamp;
    } catch (error) {
      this.tileInfoByTimestamp.delete(timestamp);
      console.error('[bpane] decode error:', error);
    }
  }

  private initDecoder(): void {
    this.closeDecoder();
    this.tileInfoByTimestamp.clear();
    this.videoDecoder = new this.videoDecoderCtor({
      output: (frame: VideoFrame) => {
        const timestamp = typeof frame.timestamp === 'number' ? frame.timestamp : null;
        const tileInfo = timestamp !== null && this.tileInfoByTimestamp.has(timestamp)
          ? this.tileInfoByTimestamp.get(timestamp) ?? null
          : null;
        if (timestamp !== null) {
          this.tileInfoByTimestamp.delete(timestamp);
        }
        this.onDecodedFrame(frame, tileInfo);
      },
      error: (error: DOMException) => {
        this.decoderConfigured = false;
        this.tileInfoByTimestamp.clear();
        this.onDecoderError(error);
      },
    });
    this.videoDecoder.configure({
      codec: 'avc1.42002a',
      optimizeForLatency: true,
    });
    this.decoderConfigured = true;
    this.decoderTimestamp = 0;
  }

  private closeDecoder(): void {
    if (!this.videoDecoder) {
      return;
    }
    try {
      this.videoDecoder.close();
    } catch (_) {
      // Ignore decoder close failures during teardown.
    }
    this.videoDecoder = null;
  }

  private buildChunk(nalData: Uint8Array, nalType: number): Uint8Array {
    if (nalType !== 5 || !this.spsNal || !this.ppsNal) {
      return nalData;
    }

    let totalLength = this.spsNal.length + this.ppsNal.length + nalData.length;
    for (const sei of this.seiNals) {
      totalLength += sei.length;
    }
    const chunk = new Uint8Array(totalLength);
    let offset = 0;
    chunk.set(this.spsNal, offset);
    offset += this.spsNal.length;
    chunk.set(this.ppsNal, offset);
    offset += this.ppsNal.length;
    for (const sei of this.seiNals) {
      chunk.set(sei, offset);
      offset += sei.length;
    }
    chunk.set(nalData, offset);
    this.seiNals = [];
    return chunk;
  }
}
