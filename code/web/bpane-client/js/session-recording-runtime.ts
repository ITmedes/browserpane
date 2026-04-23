import type { SessionRecordingOptions } from './bpane-types.js';

const DEFAULT_RECORDING_VIDEO_BITS_PER_SECOND = 3_000_000;
const DEFAULT_RECORDING_AUDIO_BITS_PER_SECOND = 96_000;
const PREFERRED_RECORDING_MIME_TYPES = [
  'video/webm;codecs=vp9,opus',
  'video/webm;codecs=vp8,opus',
  'video/webm',
];

export interface SessionRecordingRuntimeInput {
  createVideoStream: (frameRate: number) => MediaStream;
  getAudioStream: () => Promise<MediaStream | null>;
  stopVideoStream: () => void;
  mediaRecorderFactory?: (stream: MediaStream, options?: MediaRecorderOptions) => MediaRecorder;
  mediaStreamFactory?: (tracks: MediaStreamTrack[]) => MediaStream;
}

export class SessionRecordingRuntime {
  private readonly createVideoStream: (frameRate: number) => MediaStream;
  private readonly getAudioStream: () => Promise<MediaStream | null>;
  private readonly stopVideoStream: () => void;
  private readonly mediaRecorderFactory: (
    stream: MediaStream,
    options?: MediaRecorderOptions,
  ) => MediaRecorder;
  private readonly mediaStreamFactory: (tracks: MediaStreamTrack[]) => MediaStream;
  private recorder: MediaRecorder | null = null;
  private activeStream: MediaStream | null = null;
  private chunks: Blob[] = [];
  private stopPromise: Promise<Blob> | null = null;
  private lastMimeType = 'video/webm';

  constructor(input: SessionRecordingRuntimeInput) {
    this.createVideoStream = input.createVideoStream;
    this.getAudioStream = input.getAudioStream;
    this.stopVideoStream = input.stopVideoStream;
    this.mediaRecorderFactory = input.mediaRecorderFactory
      ?? ((stream, options) => new MediaRecorder(stream, options));
    this.mediaStreamFactory = input.mediaStreamFactory
      ?? ((tracks) => new MediaStream(tracks));
  }

  isRecording(): boolean {
    return this.recorder !== null;
  }

  async start(options: SessionRecordingOptions = {}): Promise<void> {
    if (this.recorder) {
      throw new Error('recording is already active');
    }

    const videoStream = this.createVideoStream(options.frameRate ?? 30);
    try {
      const audioStream = await this.getAudioStream();
      const tracks = [
        ...videoStream.getTracks(),
        ...(audioStream ? audioStream.getTracks() : []),
      ];
      const combinedStream = this.mediaStreamFactory(tracks);
      this.activeStream = combinedStream;
      const recorderOptions = this.buildRecorderOptions(options);
      this.lastMimeType = recorderOptions.mimeType ?? this.lastMimeType;
      const recorder = this.mediaRecorderFactory(combinedStream, recorderOptions);
      recorder.ondataavailable = (event) => {
        if (event.data && event.data.size > 0) {
          this.chunks.push(event.data);
        }
      };
      recorder.start();
      this.recorder = recorder;
    } catch (error) {
      this.stopVideoStream();
      throw error;
    }
  }

  async stop(): Promise<Blob> {
    if (!this.recorder) {
      throw new Error('recording is not active');
    }
    if (this.stopPromise) {
      return this.stopPromise;
    }

    const recorder = this.recorder;
    this.stopPromise = new Promise<Blob>((resolve, reject) => {
      recorder.onstop = () => {
        const blob = new Blob(this.chunks, { type: this.lastMimeType });
        this.reset();
        resolve(blob);
      };
      recorder.onerror = () => {
        this.reset();
        reject(new Error('recording failed'));
      };
    });

    if (typeof recorder.requestData === 'function') {
      recorder.requestData();
    }
    recorder.stop();
    return this.stopPromise;
  }

  destroy(): void {
    if (this.recorder) {
      try {
        if (typeof this.recorder.requestData === 'function') {
          this.recorder.requestData();
        }
        this.recorder.stop();
      } catch (_) {
        // Ignore cleanup-time recorder stop failures.
      }
    }
    this.reset();
  }

  private buildRecorderOptions(options: SessionRecordingOptions): MediaRecorderOptions {
    const recorderOptions: MediaRecorderOptions = {};
    const preferredMimeType = options.mimeType ?? this.resolvePreferredMimeType();
    if (preferredMimeType) {
      recorderOptions.mimeType = preferredMimeType;
    }
    recorderOptions.audioBitsPerSecond = typeof options.audioBitsPerSecond === 'number'
      ? options.audioBitsPerSecond
      : DEFAULT_RECORDING_AUDIO_BITS_PER_SECOND;
    recorderOptions.videoBitsPerSecond = typeof options.videoBitsPerSecond === 'number'
      ? options.videoBitsPerSecond
      : DEFAULT_RECORDING_VIDEO_BITS_PER_SECOND;
    return recorderOptions;
  }

  private resolvePreferredMimeType(): string | undefined {
    const supportsType = typeof MediaRecorder !== 'undefined'
      && typeof MediaRecorder.isTypeSupported === 'function'
      ? MediaRecorder.isTypeSupported.bind(MediaRecorder)
      : null;
    if (!supportsType) {
      return undefined;
    }
    return PREFERRED_RECORDING_MIME_TYPES.find((mimeType) => supportsType(mimeType));
  }

  private reset(): void {
    if (this.recorder) {
      this.recorder.ondataavailable = null;
      this.recorder.onstop = null;
      this.recorder.onerror = null;
    }
    if (this.activeStream) {
      this.stopTracks(this.activeStream);
      this.activeStream = null;
    }
    this.recorder = null;
    this.chunks = [];
    this.stopPromise = null;
    this.stopVideoStream();
  }

  private stopTracks(stream: MediaStream): void {
    for (const track of stream.getTracks()) {
      try {
        track.stop();
      } catch (_) {
        // Ignore cleanup-time track stop failures.
      }
    }
  }
}
