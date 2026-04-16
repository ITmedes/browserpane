import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AudioController } from '../audio-controller.js';
import { AUDIO_FRAME_HEADER_SIZE, CH_AUDIO_IN } from '../protocol.js';

const AUDIO_PAYLOAD_MAGIC = new Uint8Array([0x57, 0x52, 0x41, 0x31]);
const AUDIO_CODEC_PCM_S16LE = 0x00;
const AUDIO_CODEC_ADPCM_IMA_STEREO = 0x01;
const AUDIO_CODEC_OPUS = 0x02;

class MockPort {
  readonly postMessage = vi.fn();
  onmessage: ((event: MessageEvent) => void) | null = null;
}

class MockAudioWorkletNode {
  static instances: MockAudioWorkletNode[] = [];

  readonly port = new MockPort();
  readonly connect = vi.fn();
  readonly disconnect = vi.fn();

  constructor(
    _context: BaseAudioContext,
    _name: string,
    _options?: AudioWorkletNodeOptions,
  ) {
    MockAudioWorkletNode.instances.push(this);
  }
}

class MockMediaStreamSource {
  static instances: MockMediaStreamSource[] = [];

  readonly connect = vi.fn();

  constructor() {
    MockMediaStreamSource.instances.push(this);
  }
}

class MockAudioContext {
  static instances: MockAudioContext[] = [];

  readonly audioWorklet = {
    addModule: vi.fn(async () => {}),
  };
  readonly destination = {} as AudioDestinationNode;
  readonly createMediaStreamSource = vi.fn(
    (_stream: MediaStream) => new MockMediaStreamSource() as unknown as MediaStreamAudioSourceNode,
  );
  readonly resume = vi.fn(async () => {});
  readonly close = vi.fn(async () => {});
  state: AudioContextState = 'running';

  constructor(_options?: AudioContextOptions) {
    MockAudioContext.instances.push(this);
  }
}

class MockAudioData {
  readonly numberOfFrames: number;
  readonly numberOfChannels: number;
  readonly format: string;
  private readonly samples: Float32Array;
  readonly close = vi.fn();

  constructor(samples: Float32Array, numberOfChannels: number, format = 'f32-interleaved') {
    this.samples = samples;
    this.numberOfChannels = numberOfChannels;
    this.numberOfFrames = samples.length / numberOfChannels;
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

  copyTo(destination: AllowSharedBufferSource): void {
    if (destination instanceof Uint8Array) {
      destination.set(this.data);
      return;
    }
    new Uint8Array(destination as ArrayBufferLike).set(this.data);
  }
}

class MockAudioDecoder {
  static instances: MockAudioDecoder[] = [];

  readonly configure = vi.fn();
  readonly decode = vi.fn((chunk: EncodedAudioChunk) => {
    this.decodedChunks.push(chunk);
    this.init.output(new MockAudioData(
      new Float32Array([0.25, -0.25, 0.5, -0.5]),
      2,
    ) as unknown as AudioData);
  });
  readonly close = vi.fn();
  readonly decodedChunks: EncodedAudioChunk[] = [];
  private readonly init: AudioDecoderInit;

  constructor(init: AudioDecoderInit) {
    this.init = init;
    MockAudioDecoder.instances.push(this);
  }
}

class MockMicAudioData {
  readonly timestamp: number;
  readonly close = vi.fn();

  constructor(init: AudioDataInit) {
    this.timestamp = init.timestamp;
  }
}

class MockAudioEncoder {
  static instances: MockAudioEncoder[] = [];
  static isConfigSupported = vi.fn(async (config: AudioEncoderConfig) => ({ supported: config.codec === 'opus' }));

  readonly configure = vi.fn();
  readonly encode = vi.fn((data: AudioData) => {
    this.encodedFrames.push(data);
    this.init.output(new MockEncodedAudioChunk({
      type: 'key',
      timestamp: (data as unknown as MockMicAudioData).timestamp,
      data: new Uint8Array([9, 8, 7, 6]),
    }) as unknown as EncodedAudioChunk, {} as EncodedAudioChunkMetadata);
  });
  readonly close = vi.fn();
  readonly flush = vi.fn(async () => {});
  readonly encodedFrames: AudioData[] = [];
  encodeQueueSize = 0;
  private readonly init: AudioEncoderInit;

  constructor(init: AudioEncoderInit) {
    this.init = init;
    MockAudioEncoder.instances.push(this);
  }
}

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

function getPostedSamples(node: MockAudioWorkletNode, callIndex = 0): Float32Array {
  const [message] = node.port.postMessage.mock.calls[callIndex];
  return new Float32Array(message.samples as ArrayBuffer);
}

async function flushAsync(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('AudioController', () => {
  const trackStop = vi.fn();

  beforeEach(() => {
    MockAudioContext.instances = [];
    MockAudioWorkletNode.instances = [];
    MockMediaStreamSource.instances = [];
    MockAudioDecoder.instances = [];
    MockAudioEncoder.instances = [];
    MockAudioEncoder.isConfigSupported.mockClear();
    trackStop.mockReset();

    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.stubGlobal('AudioContext', MockAudioContext);
    vi.stubGlobal('AudioWorkletNode', MockAudioWorkletNode);
    vi.stubGlobal('AudioDecoder', MockAudioDecoder);
    vi.stubGlobal('AudioEncoder', MockAudioEncoder);
    vi.stubGlobal('AudioData', MockMicAudioData);
    vi.stubGlobal('EncodedAudioChunk', MockEncodedAudioChunk);
    Object.defineProperty(URL, 'createObjectURL', {
      configurable: true,
      writable: true,
      value: vi.fn(() => 'blob:mock-audio'),
    });
    Object.defineProperty(URL, 'revokeObjectURL', {
      configurable: true,
      writable: true,
      value: vi.fn(),
    });

    (globalThis.navigator as any).mediaDevices = {
      getUserMedia: vi.fn(async () => ({
        getTracks: () => [{ stop: trackStop }],
      }) as unknown as MediaStream),
    };
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('detects browser support for Opus microphone encoding', async () => {
    await expect(AudioController.isMicrophoneSupported()).resolves.toBe(true);
    expect(MockAudioEncoder.isConfigSupported).toHaveBeenCalledWith(expect.objectContaining({
      codec: 'opus',
      sampleRate: 48000,
      numberOfChannels: 1,
      bitrate: 32_000,
      opus: expect.objectContaining({ format: 'opus', frameDuration: 20_000 }),
    }));
  });

  it('initializes playback and decodes PCM frames', async () => {
    const controller = new AudioController(true, vi.fn());

    controller.handleFrame(new Uint8Array());
    await flushAsync();

    expect(MockAudioContext.instances).toHaveLength(1);
    expect(MockAudioContext.instances[0].audioWorklet.addModule).toHaveBeenCalledWith(
      expect.stringContaining('/audio/audio-worklet.js'),
    );
    expect(MockAudioWorkletNode.instances).toHaveLength(1);

    const pcmPayload = concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_PCM_S16LE]),
      makePcmBytes([32767, -32768, 16384, -16384]),
    );
    controller.handleFrame(makeTransportFrame(pcmPayload));

    expect(MockAudioWorkletNode.instances[0].port.postMessage).toHaveBeenCalledTimes(1);
    expect(Array.from(getPostedSamples(MockAudioWorkletNode.instances[0]))).toEqual([
      32767 / 32768,
      -1,
      0.5,
      -0.5,
    ]);
  });

  it('handles legacy PCM payloads and ADPCM stereo payloads', async () => {
    const controller = new AudioController(true, vi.fn());

    controller.handleFrame(new Uint8Array());
    await flushAsync();

    controller.handleFrame(makeTransportFrame(makePcmBytes([8192, -8192])));
    controller.handleFrame(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_ADPCM_IMA_STEREO]),
      new Uint8Array([0, 0, 0, 0, 0, 0, 0]),
    )));

    const output = MockAudioWorkletNode.instances[0];
    expect(output.port.postMessage).toHaveBeenCalledTimes(2);
    expect(Array.from(getPostedSamples(output, 0))).toEqual([0.25, -0.25]);
    expect(Array.from(getPostedSamples(output, 1))).toEqual([0, 0, 0, 0]);
  });

  it('decodes Opus payloads via AudioDecoder', async () => {
    const controller = new AudioController(true, vi.fn());

    controller.handleFrame(new Uint8Array());
    await flushAsync();

    const opusPayload = concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_OPUS, 1, 2, 3]),
    );
    controller.handleFrame(makeTransportFrame(opusPayload));
    controller.handleFrame(makeTransportFrame(opusPayload));

    expect(MockAudioDecoder.instances).toHaveLength(1);
    expect(MockAudioDecoder.instances[0].configure).toHaveBeenCalledWith({
      codec: 'opus',
      numberOfChannels: 2,
      sampleRate: 48000,
    });
    expect(MockAudioDecoder.instances[0].decodedChunks).toHaveLength(2);
    expect((MockAudioDecoder.instances[0].decodedChunks[0] as unknown as MockEncodedAudioChunk).timestamp).toBe(0);
    expect((MockAudioDecoder.instances[0].decodedChunks[1] as unknown as MockEncodedAudioChunk).timestamp).toBe(20_000);
    expect(Array.from(getPostedSamples(MockAudioWorkletNode.instances[0], 1))).toEqual([0.25, -0.25, 0.5, -0.5]);
  });

  it('captures microphone PCM and forwards it on the audio-in channel as Opus', async () => {
    const sendFrame = vi.fn();
    const controller = new AudioController(true, sendFrame);

    await controller.startMicrophone();

    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith({
      audio: { sampleRate: 48000, channelCount: 1, echoCancellation: true },
    });
    expect(MockAudioContext.instances).toHaveLength(1);
    expect(MockMediaStreamSource.instances).toHaveLength(1);
    expect(MockMediaStreamSource.instances[0].connect).toHaveBeenCalledWith(MockAudioWorkletNode.instances[0]);
    expect(MockAudioEncoder.instances).toHaveLength(1);
    expect(MockAudioEncoder.instances[0].configure).toHaveBeenCalledWith(expect.objectContaining({
      codec: 'opus',
      sampleRate: 48000,
      numberOfChannels: 1,
      bitrate: 32_000,
      opus: expect.objectContaining({ format: 'opus', frameDuration: 20_000 }),
    }));

    const pcm = new Int16Array([1000, -1000]);
    MockAudioWorkletNode.instances[0].port.onmessage?.({
      data: { type: 'pcm', data: pcm.buffer.slice(0) },
    } as MessageEvent);

    expect(sendFrame).toHaveBeenCalledTimes(1);
    expect(sendFrame.mock.calls[0][0]).toBe(CH_AUDIO_IN);

    const payload = sendFrame.mock.calls[0][1] as Uint8Array;
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    expect(view.getUint32(0, true)).toBe(1);
    expect(view.getUint32(4, true)).toBe(20_000);
    expect(view.getUint32(8, true)).toBe(0);
    expect(view.getUint32(12, true)).toBe(9);
    expect(Array.from(payload.subarray(16, 20))).toEqual(Array.from(AUDIO_PAYLOAD_MAGIC));
    expect(payload[20]).toBe(AUDIO_CODEC_OPUS);
    expect(Array.from(payload.subarray(21))).toEqual([9, 8, 7, 6]);

    controller.stopMicrophone();
    expect(MockAudioWorkletNode.instances[0].disconnect).toHaveBeenCalledTimes(1);
    expect(MockAudioEncoder.instances[0].close).toHaveBeenCalledTimes(1);
    expect(MockAudioContext.instances[0].close).toHaveBeenCalledTimes(1);
    expect(trackStop).toHaveBeenCalledTimes(1);
  });

  it('destroys playback resources and closes the Opus decoder', async () => {
    const controller = new AudioController(true, vi.fn());

    controller.handleFrame(new Uint8Array());
    await flushAsync();
    controller.handleFrame(makeTransportFrame(concatBytes(
      AUDIO_PAYLOAD_MAGIC,
      new Uint8Array([AUDIO_CODEC_OPUS, 9, 8, 7]),
    )));

    controller.destroy();

    expect(MockAudioWorkletNode.instances[0].disconnect).toHaveBeenCalledTimes(1);
    expect(MockAudioContext.instances[0].close).toHaveBeenCalledTimes(1);
    expect(MockAudioDecoder.instances[0].close).toHaveBeenCalledTimes(1);
  });
});
