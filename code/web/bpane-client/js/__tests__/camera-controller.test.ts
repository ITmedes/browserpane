import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CameraController } from '../camera-controller.js';

class MockEncodedVideoChunk {
  type: EncodedVideoChunkType;
  timestamp: number;
  byteLength: number;
  private readonly data: Uint8Array;

  constructor(type: EncodedVideoChunkType, timestamp: number, data: Uint8Array) {
    this.type = type;
    this.timestamp = timestamp;
    this.data = data;
    this.byteLength = data.byteLength;
  }

  copyTo(destination: AllowSharedBufferSource): void {
    if (destination instanceof Uint8Array) {
      destination.set(this.data);
      return;
    }
    new Uint8Array(destination as ArrayBufferLike).set(this.data);
  }
}

class MockVideoEncoder {
  static isConfigSupported = vi.fn(async (config: VideoEncoderConfig) => ({ supported: config.width! >= 640 }));
  static configured: VideoEncoderConfig[] = [];
  readonly encodeQueueSize = 0;
  private readonly output: (chunk: EncodedVideoChunk) => void;

  constructor(init: VideoEncoderInit) {
    this.output = init.output;
  }

  configure(config: VideoEncoderConfig): void {
    MockVideoEncoder.configured.push(config);
  }

  encode(_frame: VideoFrame, options?: VideoEncoderEncodeOptions): void {
    const marker = options?.keyFrame ? 0x65 : 0x41;
    this.output(new MockEncodedVideoChunk(
      options?.keyFrame ? 'key' : 'delta',
      0,
      new Uint8Array([0, 0, 0, 1, marker]),
    ) as unknown as EncodedVideoChunk);
  }

  flush(): Promise<void> {
    return Promise.resolve();
  }

  close(): void {}
}

class MockVideoFrame {
  close(): void {}
}

describe('CameraController', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockVideoEncoder.configured = [];

    const stream = {
      getTracks: () => [{
        stop: vi.fn(),
      }],
    } as unknown as MediaStream;

    (globalThis.navigator as any).mediaDevices = {
      getUserMedia: vi.fn().mockResolvedValue(stream),
    };
    (globalThis.navigator as any).mediaCapabilities = {
      encodingInfo: vi.fn(async ({ video }: { video: { width: number; height: number } }) => ({
        supported: true,
        smooth: video.width <= 1280 && video.height <= 720,
        powerEfficient: video.width <= 960,
      })),
    };

    vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined);
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
    Object.defineProperty(HTMLMediaElement.prototype, 'readyState', {
      configurable: true,
      get: () => HTMLMediaElement.HAVE_CURRENT_DATA,
    });

    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D);

    (globalThis as any).VideoEncoder = MockVideoEncoder;
    (globalThis as any).VideoFrame = MockVideoFrame;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('starts at the highest supported rung and reports telemetry', async () => {
    const sendFrame = vi.fn(() => 'sent' as const);
    const controller = new CameraController(sendFrame);

    await controller.startCamera();
    await vi.advanceTimersByTimeAsync(40);

    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith(expect.objectContaining({
      video: expect.objectContaining({
        width: { ideal: 1280 },
        height: { ideal: 720 },
        frameRate: expect.objectContaining({ ideal: 30 }),
      }),
    }));
    expect(MockVideoEncoder.configured[0]).toMatchObject({
      width: 1280,
      height: 720,
      framerate: 30,
      bitrate: 1_600_000,
    });

    const stats = controller.getStats();
    expect(stats.active).toBe(true);
    expect(stats.profile).toMatchObject({
      name: 'hd720p',
      width: 1280,
      height: 720,
      fps: 30,
      bitrate: 1_600_000,
    });
    expect(stats.framesCaptured).toBeGreaterThan(0);
    expect(stats.framesEncoded).toBeGreaterThan(0);
    expect(stats.keyframesEncoded).toBeGreaterThan(0);
  });

  it('downgrades under sustained transport replacement pressure', async () => {
    const controller = new CameraController(() => 'replaced');

    await controller.startCamera();
    await vi.advanceTimersByTimeAsync(2100);

    const stats = controller.getStats();
    expect(stats.profile).toMatchObject({
      name: 'qhd540p',
      width: 960,
      height: 540,
      fps: 24,
    });
    expect(stats.qualityLimitationReason).toBe('bandwidth');
    expect(stats.transportFramesReplaced).toBeGreaterThan(0);
    expect(stats.profileDowngrades).toBe(1);
    expect(MockVideoEncoder.configured.at(-1)).toMatchObject({
      width: 960,
      height: 540,
      framerate: 24,
      bitrate: 950_000,
    });
  });
});
