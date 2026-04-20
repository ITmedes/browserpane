import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CameraCaptureRuntime } from '../camera/camera-capture-runtime.js';
import type { CameraProfile } from '../camera/camera-profile-catalog.js';
import { CameraTelemetryTracker } from '../camera/camera-telemetry-tracker.js';

const HD_PROFILE: CameraProfile = {
  name: 'hd720p',
  width: 1280,
  height: 720,
  fps: 30,
  bitrate: 1_600_000,
  keyframeInterval: 2,
  codec: 'avc1.42001f',
  smooth: null,
  powerEfficient: null,
};

class MockEncodedVideoChunk {
  readonly type: EncodedVideoChunkType;
  readonly timestamp: number;
  readonly byteLength: number;
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
  static configured: VideoEncoderConfig[] = [];
  static instances: MockVideoEncoder[] = [];
  static nextEncodeQueueSize = 0;

  readonly encode = vi.fn((frame: VideoFrame, options?: VideoEncoderEncodeOptions) => {
    this.encodedFrames.push({
      timestamp: (frame as unknown as { timestamp: number }).timestamp,
      keyFrame: !!options?.keyFrame,
    });

    const chunk = new MockEncodedVideoChunk(
      options?.keyFrame ? 'key' : 'delta',
      (frame as unknown as { timestamp: number }).timestamp,
      new Uint8Array([0, 0, 0, 1, options?.keyFrame ? 0x65 : 0x41]),
    );
    this.output(chunk as unknown as EncodedVideoChunk);
  });
  readonly close = vi.fn();
  encodeQueueSize = 0;
  readonly encodedFrames: Array<{ timestamp: number; keyFrame: boolean }> = [];
  private readonly output: (chunk: EncodedVideoChunk) => void;
  private readonly error: (error: DOMException) => void;

  constructor(init: VideoEncoderInit) {
    this.output = init.output;
    this.error = init.error;
    this.encodeQueueSize = MockVideoEncoder.nextEncodeQueueSize;
    MockVideoEncoder.instances.push(this);
  }

  configure(config: VideoEncoderConfig): void {
    MockVideoEncoder.configured.push(config);
  }

  emitError(error: DOMException): void {
    this.error(error);
  }
}

class MockVideoFrame {
  readonly timestamp: number;
  readonly close = vi.fn();

  constructor(_source: CanvasImageSource, init: VideoFrameInit) {
    this.timestamp = init.timestamp ?? 0;
  }
}

describe('CameraCaptureRuntime', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockVideoEncoder.configured = [];
    MockVideoEncoder.instances = [];
    MockVideoEncoder.nextEncodeQueueSize = 0;

    vi.stubGlobal('VideoEncoder', MockVideoEncoder);
    vi.stubGlobal('VideoFrame', MockVideoFrame);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('configures the encoder, captures frames, and sends the encoded payloads', async () => {
    const sendFrame = vi.fn(() => 'queued' as const);
    const onEncoderError = vi.fn();
    const videoElement = document.createElement('video');
    Object.defineProperty(videoElement, 'readyState', {
      configurable: true,
      get: () => HTMLMediaElement.HAVE_CURRENT_DATA,
    });
    const canvasElement = document.createElement('canvas');
    const canvasContext = { drawImage: vi.fn() } as unknown as CanvasRenderingContext2D;
    const telemetry = new CameraTelemetryTracker();
    const runtime = new CameraCaptureRuntime({
      videoElement,
      canvasElement,
      canvasContext,
      sendFrame,
      telemetry,
      onEncoderError,
    });

    runtime.applyProfile(HD_PROFILE);
    await vi.advanceTimersByTimeAsync(40);

    expect(MockVideoEncoder.configured[0]).toMatchObject({
      width: 1280,
      height: 720,
      framerate: 30,
      bitrate: 1_600_000,
    });
    expect(canvasElement.width).toBe(1280);
    expect(canvasElement.height).toBe(720);
    expect(canvasContext.drawImage).toHaveBeenCalled();
    expect(sendFrame).toHaveBeenCalled();
    expect(telemetry.getMetrics()).toMatchObject({
      framesCaptured: 1,
      framesEncoded: 1,
      keyframesEncoded: 1,
      transportFramesQueued: 1,
    });
    expect(onEncoderError).not.toHaveBeenCalled();

    runtime.stop();
  });

  it('resets timestamp and keyframe cadence when a profile is reapplied', async () => {
    const videoElement = document.createElement('video');
    Object.defineProperty(videoElement, 'readyState', {
      configurable: true,
      get: () => HTMLMediaElement.HAVE_CURRENT_DATA,
    });
    const runtime = new CameraCaptureRuntime({
      videoElement,
      canvasElement: document.createElement('canvas'),
      canvasContext: { drawImage: vi.fn() } as unknown as CanvasRenderingContext2D,
      sendFrame: () => 'sent',
      telemetry: new CameraTelemetryTracker(),
      onEncoderError: vi.fn(),
    });

    runtime.applyProfile(HD_PROFILE);
    await vi.advanceTimersByTimeAsync(80);
    runtime.applyProfile(HD_PROFILE);
    await vi.advanceTimersByTimeAsync(40);

    expect(MockVideoEncoder.instances).toHaveLength(2);
    expect(MockVideoEncoder.instances[0].close).toHaveBeenCalledTimes(1);
    expect(MockVideoEncoder.instances[0].encodedFrames).toEqual([
      { timestamp: 0, keyFrame: true },
      { timestamp: Math.round(1_000_000 / HD_PROFILE.fps), keyFrame: false },
    ]);
    expect(MockVideoEncoder.instances[1].encodedFrames[0]).toEqual({
      timestamp: 0,
      keyFrame: true,
    });

    runtime.stop();
  });

  it('records queue drops instead of encoding when the encoder queue is backed up', async () => {
    MockVideoEncoder.nextEncodeQueueSize = 3;
    const telemetry = new CameraTelemetryTracker();
    const videoElement = document.createElement('video');
    Object.defineProperty(videoElement, 'readyState', {
      configurable: true,
      get: () => HTMLMediaElement.HAVE_CURRENT_DATA,
    });
    const runtime = new CameraCaptureRuntime({
      videoElement,
      canvasElement: document.createElement('canvas'),
      canvasContext: { drawImage: vi.fn() } as unknown as CanvasRenderingContext2D,
      sendFrame: vi.fn(() => 'sent' as const),
      telemetry,
      onEncoderError: vi.fn(),
    });

    runtime.applyProfile(HD_PROFILE);
    await vi.advanceTimersByTimeAsync(40);

    expect(MockVideoEncoder.instances[0].encode).not.toHaveBeenCalled();
    expect(telemetry.getMetrics()).toMatchObject({
      encoderQueueDrops: 1,
      framesCaptured: 0,
      framesEncoded: 0,
    });

    runtime.stop();
  });

  it('forwards encoder errors through the provided callback', () => {
    const onEncoderError = vi.fn();
    const runtime = new CameraCaptureRuntime({
      videoElement: document.createElement('video'),
      canvasElement: document.createElement('canvas'),
      canvasContext: { drawImage: vi.fn() } as unknown as CanvasRenderingContext2D,
      sendFrame: () => 'sent',
      telemetry: new CameraTelemetryTracker(),
      onEncoderError,
    });

    runtime.applyProfile(HD_PROFILE);
    const error = new DOMException('boom');
    MockVideoEncoder.instances[0].emitError(error);

    expect(onEncoderError).toHaveBeenCalledWith(error);

    runtime.stop();
  });
});
