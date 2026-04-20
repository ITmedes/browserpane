import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CH_AUDIO_IN } from '../protocol.js';
import { MicrophoneRuntime } from '../audio/microphone-runtime.js';

const AUDIO_PAYLOAD_MAGIC = new Uint8Array([0x57, 0x52, 0x41, 0x31]);
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
  static nextAddModuleError: Error | null = null;

  readonly audioWorklet = {
    addModule: vi.fn(async () => {
      if (MockAudioContext.nextAddModuleError) {
        const error = MockAudioContext.nextAddModuleError;
        MockAudioContext.nextAddModuleError = null;
        throw error;
      }
    }),
  };
  readonly createMediaStreamSource = vi.fn(
    (_stream: MediaStream) => new MockMediaStreamSource() as unknown as MediaStreamAudioSourceNode,
  );
  readonly close = vi.fn(async () => {});

  constructor(_options?: AudioContextOptions) {
    MockAudioContext.instances.push(this);
  }
}

class MockMicAudioData {
  readonly timestamp: number;
  readonly close = vi.fn();

  constructor(init: AudioDataInit) {
    this.timestamp = init.timestamp;
  }
}

class MockEncodedAudioChunk {
  readonly data: Uint8Array;
  readonly byteLength: number;

  constructor(init: EncodedAudioChunkInit) {
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

class MockAudioEncoder {
  static instances: MockAudioEncoder[] = [];
  static isConfigSupported = vi.fn(async (config: AudioEncoderConfig) => ({
    supported: config.codec === 'opus',
  }));

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
  readonly encodedFrames: AudioData[] = [];
  encodeQueueSize = 0;
  private readonly init: AudioEncoderInit;

  constructor(init: AudioEncoderInit) {
    this.init = init;
    MockAudioEncoder.instances.push(this);
  }
}

function makeMicPcm(samples: number[]): ArrayBuffer {
  return new Int16Array(samples).buffer.slice(0);
}

describe('MicrophoneRuntime', () => {
  const trackStop = vi.fn();

  beforeEach(() => {
    MockAudioContext.instances = [];
    MockAudioContext.nextAddModuleError = null;
    MockAudioWorkletNode.instances = [];
    MockMediaStreamSource.instances = [];
    MockAudioEncoder.instances = [];
    MockAudioEncoder.isConfigSupported.mockClear();
    trackStop.mockReset();

    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.stubGlobal('AudioContext', MockAudioContext);
    vi.stubGlobal('AudioWorkletNode', MockAudioWorkletNode);
    vi.stubGlobal('AudioEncoder', MockAudioEncoder);
    vi.stubGlobal('AudioData', MockMicAudioData);
    vi.stubGlobal('EncodedAudioChunk', MockEncodedAudioChunk);
    Object.defineProperty(URL, 'createObjectURL', {
      configurable: true,
      writable: true,
      value: vi.fn(() => 'blob:mock-mic'),
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

  it('captures microphone PCM and forwards it on the audio-in channel as Opus', async () => {
    const sendFrame = vi.fn();
    const runtime = new MicrophoneRuntime(sendFrame);

    await runtime.start();

    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith({
      audio: { sampleRate: 48000, channelCount: 1, echoCancellation: true },
    });
    expect(MockAudioContext.instances).toHaveLength(1);
    expect(MockMediaStreamSource.instances).toHaveLength(1);
    expect(MockMediaStreamSource.instances[0].connect).toHaveBeenCalledWith(MockAudioWorkletNode.instances[0]);
    expect(MockAudioEncoder.instances).toHaveLength(1);

    MockAudioWorkletNode.instances[0].port.onmessage?.({
      data: { type: 'pcm', data: makeMicPcm([1000, -1000]) },
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

    runtime.stop();
    expect(MockAudioWorkletNode.instances[0].disconnect).toHaveBeenCalledTimes(1);
    expect(MockAudioEncoder.instances[0].close).toHaveBeenCalledTimes(1);
    expect(MockAudioContext.instances[0].close).toHaveBeenCalledTimes(1);
    expect(trackStop).toHaveBeenCalledTimes(1);
  });

  it('drops microphone PCM when the encoder queue is backpressured and advances timestamps', async () => {
    const sendFrame = vi.fn();
    const runtime = new MicrophoneRuntime(sendFrame);

    await runtime.start();

    MockAudioEncoder.instances[0].encodeQueueSize = 4;
    MockAudioWorkletNode.instances[0].port.onmessage?.({
      data: { type: 'pcm', data: makeMicPcm([1000, -1000]) },
    } as MessageEvent);

    expect(sendFrame).not.toHaveBeenCalled();
    expect(MockAudioEncoder.instances[0].encode).not.toHaveBeenCalled();

    MockAudioEncoder.instances[0].encodeQueueSize = 0;
    MockAudioWorkletNode.instances[0].port.onmessage?.({
      data: { type: 'pcm', data: makeMicPcm([2000, -2000]) },
    } as MessageEvent);

    expect(sendFrame).toHaveBeenCalledTimes(1);
    expect((MockAudioEncoder.instances[0].encodedFrames[0] as unknown as MockMicAudioData).timestamp).toBe(20_000);

    const payload = sendFrame.mock.calls[0][1] as Uint8Array;
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    expect(view.getUint32(0, true)).toBe(1);
    expect(view.getUint32(4, true)).toBe(20_000);
  });

  it('cleans up partial startup when microphone worklet bootstrap fails', async () => {
    const sendFrame = vi.fn();
    const runtime = new MicrophoneRuntime(sendFrame);
    MockAudioContext.nextAddModuleError = new Error('worklet failed');

    await expect(runtime.start()).resolves.toBeUndefined();

    expect(sendFrame).not.toHaveBeenCalled();
    expect(MockAudioEncoder.instances).toHaveLength(1);
    expect(MockAudioEncoder.instances[0].close).toHaveBeenCalledTimes(1);
    expect(MockAudioContext.instances[0].close).toHaveBeenCalledTimes(1);
    expect(trackStop).toHaveBeenCalledTimes(1);
    expect(console.error).toHaveBeenCalledWith('[bpane] microphone error:', expect.any(Error));
  });
});
