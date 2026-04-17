import { CameraProfileCatalog } from './camera-profile-catalog.js';

export interface CameraSessionResourceSnapshot {
  stream: MediaStream;
  videoElement: HTMLVideoElement;
  canvasElement: HTMLCanvasElement;
  canvasContext: CanvasRenderingContext2D;
}

export class CameraSessionResources {
  private snapshot: CameraSessionResourceSnapshot | null = null;

  async open(): Promise<CameraSessionResourceSnapshot> {
    if (this.snapshot) {
      return this.snapshot;
    }

    let stream: MediaStream | null = null;
    let videoElement: HTMLVideoElement | null = null;
    try {
      stream = await navigator.mediaDevices.getUserMedia(
        CameraProfileCatalog.getCaptureConstraints(),
      );

      videoElement = document.createElement('video');
      videoElement.muted = true;
      videoElement.playsInline = true;
      videoElement.autoplay = true;
      videoElement.srcObject = stream;

      const canvasElement = document.createElement('canvas');
      const canvasContext = canvasElement.getContext('2d');
      if (!canvasContext) {
        throw new Error('camera canvas context unavailable');
      }

      this.snapshot = {
        stream,
        videoElement,
        canvasElement,
        canvasContext,
      };

      await videoElement.play();
      return this.snapshot;
    } catch (error) {
      if (this.snapshot) {
        this.close();
      } else {
        stream?.getTracks().forEach((track) => track.stop());
        if (videoElement) {
          videoElement.pause();
          videoElement.srcObject = null;
        }
      }
      throw error;
    }
  }

  close(): void {
    if (!this.snapshot) {
      return;
    }

    this.snapshot.stream.getTracks().forEach((track) => track.stop());
    this.snapshot.videoElement.pause();
    this.snapshot.videoElement.srcObject = null;
    this.snapshot = null;
  }

  getSnapshot(): CameraSessionResourceSnapshot | null {
    return this.snapshot;
  }
}
