import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { SessionRecordingRuntime } from '../session-recording-runtime.js';

class MockTrack {
  readonly kind: string;
  readonly stop = vi.fn();

  constructor(kind: string) {
    this.kind = kind;
  }
}

class MockMediaStream {
  private readonly tracks: any[];

  constructor(tracks: any[] = []) {
    this.tracks = [...tracks];
  }

  addTrack(track: any): void {
    this.tracks.push(track);
  }

  getTracks(): any[] {
    return [...this.tracks];
  }

  getAudioTracks(): any[] {
    return this.tracks.filter((track) => track.kind === 'audio');
  }

  getVideoTracks(): any[] {
    return this.tracks.filter((track) => track.kind === 'video');
  }
}

class MockBlobEvent extends Event {
  readonly data: Blob;

  constructor(type: string, data: Blob) {
    super(type);
    this.data = data;
  }
}

class MockMediaRecorder extends EventTarget {
  static instances: MockMediaRecorder[] = [];

  readonly stream: MockMediaStream;
  readonly options?: MediaRecorderOptions;
  readonly start = vi.fn();
  readonly stop = vi.fn();
  readonly requestData = vi.fn();
  state: RecordingState = 'inactive';
  ondataavailable: ((event: BlobEvent) => void) | null = null;
  onstop: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  constructor(stream: MockMediaStream, options?: MediaRecorderOptions) {
    super();
    this.stream = stream;
    this.options = options;
    MockMediaRecorder.instances.push(this);
  }

  emitData(bytes: number[]): void {
    const event = new MockBlobEvent(
      'dataavailable',
      new Blob([new Uint8Array(bytes)], { type: this.options?.mimeType ?? 'video/webm' }),
    ) as unknown as BlobEvent;
    this.ondataavailable?.(event);
    this.dispatchEvent(event);
  }

  emitStop(): void {
    this.state = 'inactive';
    const event = new Event('stop');
    this.onstop?.(event);
    this.dispatchEvent(event);
  }
}

describe('SessionRecordingRuntime', () => {
  beforeEach(() => {
    MockMediaRecorder.instances = [];
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('starts a programmatic recording with combined video and audio tracks and returns a blob on stop', async () => {
    const stopVideoStream = vi.fn();
    const runtime = new SessionRecordingRuntime({
      createVideoStream: vi.fn(() => new MockMediaStream([new MockTrack('video')]) as unknown as MediaStream),
      getAudioStream: vi.fn(async () => new MockMediaStream([new MockTrack('audio')]) as unknown as MediaStream),
      stopVideoStream,
      mediaRecorderFactory: (stream, options) => new MockMediaRecorder(
        stream as unknown as MockMediaStream,
        options,
      ) as unknown as MediaRecorder,
      mediaStreamFactory: (tracks) => new MockMediaStream(tracks as any[]) as unknown as MediaStream,
    });

    await runtime.start({ frameRate: 24, mimeType: 'video/webm;codecs=vp9,opus' });

    expect(runtime.isRecording()).toBe(true);
    expect(MockMediaRecorder.instances).toHaveLength(1);
    expect(MockMediaRecorder.instances[0].start).toHaveBeenCalledOnce();
    expect(MockMediaRecorder.instances[0].stream.getVideoTracks()).toHaveLength(1);
    expect(MockMediaRecorder.instances[0].stream.getAudioTracks()).toHaveLength(1);

    MockMediaRecorder.instances[0].emitData([1, 2, 3, 4]);
    const stopPromise = runtime.stop();
    expect(MockMediaRecorder.instances[0].requestData).toHaveBeenCalledOnce();
    expect(MockMediaRecorder.instances[0].stop).toHaveBeenCalledOnce();
    MockMediaRecorder.instances[0].emitStop();

    const blob = await stopPromise;
    expect(blob.size).toBe(4);
    expect(runtime.isRecording()).toBe(false);
    expect(stopVideoStream).toHaveBeenCalledOnce();
  });

  it('records video-only when no audio stream is available', async () => {
    const runtime = new SessionRecordingRuntime({
      createVideoStream: vi.fn(() => new MockMediaStream([new MockTrack('video')]) as unknown as MediaStream),
      getAudioStream: vi.fn(async () => null),
      stopVideoStream: vi.fn(),
      mediaRecorderFactory: (stream, options) => new MockMediaRecorder(
        stream as unknown as MockMediaStream,
        options,
      ) as unknown as MediaRecorder,
      mediaStreamFactory: (tracks) => new MockMediaStream(tracks as any[]) as unknown as MediaStream,
    });

    await runtime.start();

    expect(MockMediaRecorder.instances[0].stream.getVideoTracks()).toHaveLength(1);
    expect(MockMediaRecorder.instances[0].stream.getAudioTracks()).toHaveLength(0);
  });

  it('uses a compressed default recording profile when no explicit options are provided', async () => {
    const isTypeSupported = vi.fn((mimeType: string) => mimeType === 'video/webm;codecs=vp9,opus');
    (globalThis as any).MediaRecorder = Object.assign(
      class {},
      { isTypeSupported },
    );

    const runtime = new SessionRecordingRuntime({
      createVideoStream: vi.fn(() => new MockMediaStream([new MockTrack('video')]) as unknown as MediaStream),
      getAudioStream: vi.fn(async () => new MockMediaStream([new MockTrack('audio')]) as unknown as MediaStream),
      stopVideoStream: vi.fn(),
      mediaRecorderFactory: (stream, options) => new MockMediaRecorder(
        stream as unknown as MockMediaStream,
        options,
      ) as unknown as MediaRecorder,
      mediaStreamFactory: (tracks) => new MockMediaStream(tracks as any[]) as unknown as MediaStream,
    });

    await runtime.start();

    expect(MockMediaRecorder.instances[0].options).toEqual(expect.objectContaining({
      mimeType: 'video/webm;codecs=vp9,opus',
      videoBitsPerSecond: 3_000_000,
      audioBitsPerSecond: 96_000,
    }));
    expect(isTypeSupported).toHaveBeenCalled();
  });

  it('stops the recording tracks during destroy cleanup', async () => {
    const videoTrack = new MockTrack('video');
    const audioTrack = new MockTrack('audio');
    const runtime = new SessionRecordingRuntime({
      createVideoStream: vi.fn(() => new MockMediaStream([videoTrack]) as unknown as MediaStream),
      getAudioStream: vi.fn(async () => new MockMediaStream([audioTrack]) as unknown as MediaStream),
      stopVideoStream: vi.fn(),
      mediaRecorderFactory: (stream, options) => new MockMediaRecorder(
        stream as unknown as MockMediaStream,
        options,
      ) as unknown as MediaRecorder,
      mediaStreamFactory: (tracks) => new MockMediaStream(tracks as any[]) as unknown as MediaStream,
    });

    await runtime.start();
    runtime.destroy();

    expect(MockMediaRecorder.instances[0].stop).toHaveBeenCalledOnce();
    expect(videoTrack.stop).toHaveBeenCalledOnce();
    expect(audioTrack.stop).toHaveBeenCalledOnce();
    expect(runtime.isRecording()).toBe(false);
  });
});
