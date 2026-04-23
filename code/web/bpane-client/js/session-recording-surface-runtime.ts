export interface SessionRecordingSurfaceRuntimeInput {
  sourceCanvas: HTMLCanvasElement;
  cursorCanvas: HTMLCanvasElement | null;
}

export class SessionRecordingSurfaceRuntime {
  private readonly sourceCanvas: HTMLCanvasElement;
  private readonly cursorCanvas: HTMLCanvasElement | null;
  private recordingCanvas: HTMLCanvasElement | null = null;
  private recordingContext: CanvasRenderingContext2D | null = null;
  private recordingStream: MediaStream | null = null;
  private animationFrameId: number | null = null;

  constructor(input: SessionRecordingSurfaceRuntimeInput) {
    this.sourceCanvas = input.sourceCanvas;
    this.cursorCanvas = input.cursorCanvas;
  }

  start(frameRate = 30): MediaStream {
    if (this.recordingStream) {
      return this.recordingStream;
    }

    const recordingCanvas = document.createElement('canvas');
    recordingCanvas.width = this.sourceCanvas.width;
    recordingCanvas.height = this.sourceCanvas.height;
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
    this.drawFrame();
    this.recordingStream = captureStream.call(recordingCanvas, frameRate);
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
    this.recordingContext = null;
    this.recordingCanvas = null;
  }

  private drawFrame = (): void => {
    const canvas = this.recordingCanvas;
    const context = this.recordingContext;
    if (!canvas || !context) {
      return;
    }

    if (canvas.width !== this.sourceCanvas.width || canvas.height !== this.sourceCanvas.height) {
      canvas.width = this.sourceCanvas.width;
      canvas.height = this.sourceCanvas.height;
    }

    context.clearRect(0, 0, canvas.width, canvas.height);
    context.drawImage(this.sourceCanvas, 0, 0, canvas.width, canvas.height);
    if (this.cursorCanvas) {
      context.drawImage(this.cursorCanvas, 0, 0, canvas.width, canvas.height);
    }

    this.animationFrameId = requestAnimationFrame(this.drawFrame);
  };
}
