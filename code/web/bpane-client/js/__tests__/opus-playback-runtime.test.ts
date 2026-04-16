import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { OpusPlaybackRuntime } from '../audio/opus-playback-runtime.js';

type OutputDescriptor = {
  samples: Float32Array;
  numberOfChannels: number;
  format?: string;
};

class MockAudioData {
  readonly numberOfFrames: number;
  readonly numberOfChannels: number;
  readonly format: string;
  private readonly samples: Float32Array;
  readonly close = vi.fn();

  constructor(samples: Float32Array, numberOfChannels: number, format = 'f32-interleaved') {
    this.samples = samples;
    this.numberOfChannels = numberOfChannels;
    this.numberOfFrames = numberOfChannels > 0 ? samples.length / numberOfChannels : 0;
    this.format = format;
  }

  copyTo(destination: AllowSharedBufferSource, _options?: AudioDataCopyToOptions): void {
    if (destination instanceof Float32Array) {
      destination.set(this.samples);
      return;
    }
    new Float32Array(destination as ArrayBufferLike).set(this.samples);
  }
}

class MockEncodedAudioChunk {
  readonly type: EncodedAudioChunkType;
  readonly timestamp: number;
  readonly data: Uint8Array;
  readonly byteLength: number;

  constructor(init: EncodedAudioChunkInit) {
    this.type = init.type;
    this.timestamp = init.timestamp;
    this.data = new Uint8Array(init.data as ArrayBufferLike);
    this.byteLength = this.data.byteLength;
  }
}

class MockAudioDecoder {
  static instances: MockAudioDecoder[] = [];
  static outputs: OutputDescriptor[] = [];

  readonly configure = vi.fn();
  readonly decode = vi.fn((chunk: EncodedAudioChunk) => {
    this.decodedChunks.push(chunk);
    const next = MockAudioDecoder.outputs.shift() ?? {
      samples: new Float32Array([0.25, -0.25, 0.5, -0.5]),
      numberOfChannels: 2,
      format: 'f32-interleaved',
    };
    this.init.output(new MockAudioData(
      next.samples,
      next.numberOfChannels,
      next.format,
    ) as unknown as AudioData);
  });
  readonly close = vi.fn();
  readonly decodedChunks: EncodedAudioChunk[] = [];
  private readonly init: AudioDecoderInit;

  constructor(init: AudioDecoderInit) {
    this.init = init;
    MockAudioDecoder.instances.push(this);
  }

  emitError(message: string): void {
    this.init.error(new DOMException(message));
  }
}

describe('OpusPlaybackRuntime', () => {
  beforeEach(() => {
    MockAudioDecoder.instances = [];
    MockAudioDecoder.outputs = [];

    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.stubGlobal('AudioDecoder', MockAudioDecoder);
    vi.stubGlobal('EncodedAudioChunk', MockEncodedAudioChunk);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('decodes Opus packets and forwards stereo float samples', () => {
    const onSamples = vi.fn();
    const runtime = new OpusPlaybackRuntime(onSamples);

    runtime.decode(new Uint8Array([1, 2, 3]));
    runtime.decode(new Uint8Array([4, 5, 6]));

    expect(MockAudioDecoder.instances).toHaveLength(1);
    expect(MockAudioDecoder.instances[0].configure).toHaveBeenCalledWith({
      codec: 'opus',
      numberOfChannels: 2,
      sampleRate: 48000,
    });
    expect(MockAudioDecoder.instances[0].decodedChunks).toHaveLength(2);
    expect((MockAudioDecoder.instances[0].decodedChunks[0] as unknown as MockEncodedAudioChunk).timestamp).toBe(0);
    expect((MockAudioDecoder.instances[0].decodedChunks[1] as unknown as MockEncodedAudioChunk).timestamp).toBe(20_000);
    expect(Array.from(onSamples.mock.calls[0][0] as Float32Array)).toEqual([0.25, -0.25, 0.5, -0.5]);
  });

  it('expands mono decoded frames to stereo samples', () => {
    const onSamples = vi.fn();
    const runtime = new OpusPlaybackRuntime(onSamples);
    MockAudioDecoder.outputs.push({
      samples: new Float32Array([0.25, 0.5]),
      numberOfChannels: 1,
      format: 'f32-interleaved',
    });

    runtime.decode(new Uint8Array([1, 2, 3]));

    expect(Array.from(onSamples.mock.calls[0][0] as Float32Array)).toEqual([0.25, 0.25, 0.5, 0.5]);
  });

  it('ignores empty decoded frames', () => {
    const onSamples = vi.fn();
    const runtime = new OpusPlaybackRuntime(onSamples);
    MockAudioDecoder.outputs.push({
      samples: new Float32Array([]),
      numberOfChannels: 2,
      format: 'f32-interleaved',
    });

    runtime.decode(new Uint8Array([1, 2, 3]));

    expect(onSamples).not.toHaveBeenCalled();
  });

  it('recreates the decoder after a decoder error', () => {
    const onSamples = vi.fn();
    const runtime = new OpusPlaybackRuntime(onSamples);

    runtime.decode(new Uint8Array([1, 2, 3]));
    MockAudioDecoder.instances[0].emitError('decode failed');
    runtime.decode(new Uint8Array([4, 5, 6]));

    expect(console.error).toHaveBeenCalledWith('[bpane] Opus AudioDecoder error:', 'decode failed');
    expect(MockAudioDecoder.instances).toHaveLength(2);
    expect((MockAudioDecoder.instances[1].decodedChunks[0] as unknown as MockEncodedAudioChunk).timestamp).toBe(0);
  });

  it('closes the decoder and resets timestamps on destroy', () => {
    const onSamples = vi.fn();
    const runtime = new OpusPlaybackRuntime(onSamples);

    runtime.decode(new Uint8Array([1, 2, 3]));
    runtime.destroy();
    runtime.decode(new Uint8Array([4, 5, 6]));

    expect(MockAudioDecoder.instances[0].close).toHaveBeenCalledTimes(1);
    expect(MockAudioDecoder.instances).toHaveLength(2);
    expect((MockAudioDecoder.instances[1].decodedChunks[0] as unknown as MockEncodedAudioChunk).timestamp).toBe(0);
  });
});
