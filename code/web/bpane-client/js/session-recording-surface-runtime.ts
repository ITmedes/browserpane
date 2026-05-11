export interface SessionRecordingSurfaceRuntimeInput {
  sourceCanvas: HTMLCanvasElement;
  cursorCanvas: HTMLCanvasElement | null;
}

type CanvasCaptureTrack = MediaStreamTrack & { requestFrame?: () => void };

export class SessionRecordingSurfaceRuntime {
  private readonly sourceCanvas: HTMLCanvasElement;
  private readonly cursorCanvas: HTMLCanvasElement | null;
  private recordingCanvas: HTMLCanvasElement | null = null;
  private recordingContext: CanvasRenderingContext2D | null = null;
  private recordingStream: MediaStream | null = null;
  private recordingTrack: CanvasCaptureTrack | null = null;
  private animationFrameId: number | null = null;
  private captureFrameIntervalMs = 1000 / 30;
  private lastCaptureFrameAtMs = Number.NEGATIVE_INFINITY;

  constructor(input: SessionRecordingSurfaceRuntimeInput) {
    this.sourceCanvas = input.sourceCanvas;
    this.cursorCanvas = input.cursorCanvas;
  }

  start(frameRate = 30): MediaStream {
    if (this.recordingStream) {
      return this.recordingStream;
    }

    const recordingCanvas = document.createElement('canvas');
    const outputSize = this.getOutputSize();
    recordingCanvas.width = outputSize.width;
    recordingCanvas.height = outputSize.height;
    const recordingContext = recordingCanvas.getContext('2d');
    if (!recordingContext) {
      throw new Error('2d recording canvas is unavailable');
    }
    const captureStream = (recordingCanvas as HTMLCanvasElement & {
      captureStream?: (fps?: number) => MediaStream;
    }).captureStream;
    if (typeof captureStream !== 'function') {
      throw new Error('canvas captureStream is unavailable in this browser');
    }

    this.recordingCanvas = recordingCanvas;
    this.recordingContext = recordingContext;
    this.captureFrameIntervalMs = 1000 / Math.max(1, frameRate);
    this.lastCaptureFrameAtMs = Number.NEGATIVE_INFINITY;
    this.recordingStream = captureStream.call(recordingCanvas, frameRate);
    const getVideoTracks = this.recordingStream.getVideoTracks?.bind(this.recordingStream);
    this.recordingTrack = getVideoTracks?.()[0] as CanvasCaptureTrack | undefined ?? null;
    this.drawFrame();
    return this.recordingStream;
  }

  stop(): void {
    if (this.animationFrameId !== null) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
    if (this.recordingStream) {
      for (const track of this.recordingStream.getTracks()) {
        try {
          track.stop();
        } catch (_) {
          // Ignore cleanup-time capture track failures.
        }
      }
    }
    this.recordingStream = null;
    this.recordingTrack = null;
    this.recordingContext = null;
    this.recordingCanvas = null;
  }

  private drawFrame = (now = performance.now()): void => {
    const canvas = this.recordingCanvas;
    const context = this.recordingContext;
    if (!canvas || !context) {
      return;
    }

    const outputSize = this.getOutputSize();
    if (canvas.width !== outputSize.width || canvas.height !== outputSize.height) {
      canvas.width = outputSize.width;
      canvas.height = outputSize.height;
    }

    context.clearRect(0, 0, canvas.width, canvas.height);
    context.drawImage(this.sourceCanvas, 0, 0, canvas.width, canvas.height);
    if (this.cursorCanvas) {
      context.drawImage(this.cursorCanvas, 0, 0, canvas.width, canvas.height);
    }
    this.requestCaptureFrame(now);

    this.animationFrameId = requestAnimationFrame(this.drawFrame);
  };

  private requestCaptureFrame(now: number): void {
    if (now - this.lastCaptureFrameAtMs < this.captureFrameIntervalMs) {
      return;
    }
    try {
      this.recordingTrack?.requestFrame?.();
      this.lastCaptureFrameAtMs = now;
    } catch (_) {
      // Ignore browser-specific requestFrame failures; the timed capture stream is still active.
    }
  }

  private getOutputSize(): { width: number; height: number } {
    const sourceRect = this.sourceCanvas.getBoundingClientRect();
    const width = Math.max(1, this.sourceCanvas.width || Math.round(sourceRect.width));
    const height = Math.max(1, this.sourceCanvas.height || Math.round(sourceRect.height));
    return { width, height };
  }
}
