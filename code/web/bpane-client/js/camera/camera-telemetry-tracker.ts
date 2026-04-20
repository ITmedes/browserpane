import type { CameraAdaptationWindowStats } from './camera-adaptation-policy.js';

type CameraSendResult = 'sent' | 'queued' | 'replaced';

export interface CameraTelemetryMetrics {
  framesCaptured: number;
  framesEncoded: number;
  keyframesEncoded: number;
  encodedBytes: number;
  transportFramesQueued: number;
  transportFramesReplaced: number;
  encoderQueueDrops: number;
  averageEncodeTimeMs: number;
  maxEncodeTimeMs: number;
  profileUpgrades: number;
  profileDowngrades: number;
  reconfigurations: number;
}

export class CameraTelemetryTracker {
  private framesCaptured = 0;
  private framesEncoded = 0;
  private keyframesEncoded = 0;
  private encodedBytes = 0;
  private transportFramesQueued = 0;
  private transportFramesReplaced = 0;
  private encoderQueueDrops = 0;
  private totalEncodeTimeMs = 0;
  private maxEncodeTimeMs = 0;
  private profileUpgrades = 0;
  private profileDowngrades = 0;
  private reconfigurations = 0;

  private windowTransportQueued = 0;
  private windowTransportReplaced = 0;
  private windowEncoderQueueDrops = 0;
  private windowEncodeTimeMs = 0;
  private windowEncodedSamples = 0;

  reset(): void {
    this.framesCaptured = 0;
    this.framesEncoded = 0;
    this.keyframesEncoded = 0;
    this.encodedBytes = 0;
    this.transportFramesQueued = 0;
    this.transportFramesReplaced = 0;
    this.encoderQueueDrops = 0;
    this.totalEncodeTimeMs = 0;
    this.maxEncodeTimeMs = 0;
    this.profileUpgrades = 0;
    this.profileDowngrades = 0;
    this.reconfigurations = 0;
    this.resetWindow();
  }

  resetWindow(): void {
    this.windowTransportQueued = 0;
    this.windowTransportReplaced = 0;
    this.windowEncoderQueueDrops = 0;
    this.windowEncodeTimeMs = 0;
    this.windowEncodedSamples = 0;
  }

  recordProfileChange(previousIndex: number, nextIndex: number): void {
    if (previousIndex < 0 || previousIndex === nextIndex) {
      return;
    }

    this.reconfigurations += 1;
    if (nextIndex > previousIndex) {
      this.profileDowngrades += 1;
      return;
    }

    this.profileUpgrades += 1;
  }

  recordCapture(): void {
    this.framesCaptured += 1;
  }

  recordEncoderQueueDrop(): void {
    this.encoderQueueDrops += 1;
    this.windowEncoderQueueDrops += 1;
  }

  recordEncodedChunk(input: {
    chunkType: EncodedVideoChunkType;
    chunkByteLength: number;
    sendResult: CameraSendResult;
    encodeTimeMs?: number;
  }): void {
    if (typeof input.encodeTimeMs === 'number') {
      this.totalEncodeTimeMs += input.encodeTimeMs;
      this.windowEncodeTimeMs += input.encodeTimeMs;
      this.windowEncodedSamples += 1;
      this.maxEncodeTimeMs = Math.max(this.maxEncodeTimeMs, input.encodeTimeMs);
    }

    this.framesEncoded += 1;
    if (input.chunkType === 'key') {
      this.keyframesEncoded += 1;
    }
    this.encodedBytes += input.chunkByteLength;

    if (input.sendResult === 'queued') {
      this.transportFramesQueued += 1;
      this.windowTransportQueued += 1;
    } else if (input.sendResult === 'replaced') {
      this.transportFramesReplaced += 1;
      this.windowTransportReplaced += 1;
    }
  }

  getFramesEncoded(): number {
    return this.framesEncoded;
  }

  getMetrics(): CameraTelemetryMetrics {
    return {
      framesCaptured: this.framesCaptured,
      framesEncoded: this.framesEncoded,
      keyframesEncoded: this.keyframesEncoded,
      encodedBytes: this.encodedBytes,
      transportFramesQueued: this.transportFramesQueued,
      transportFramesReplaced: this.transportFramesReplaced,
      encoderQueueDrops: this.encoderQueueDrops,
      averageEncodeTimeMs: this.framesEncoded > 0 ? this.totalEncodeTimeMs / this.framesEncoded : 0,
      maxEncodeTimeMs: this.maxEncodeTimeMs,
      profileUpgrades: this.profileUpgrades,
      profileDowngrades: this.profileDowngrades,
      reconfigurations: this.reconfigurations,
    };
  }

  getWindowStats(): CameraAdaptationWindowStats {
    return {
      transportQueued: this.windowTransportQueued,
      transportReplaced: this.windowTransportReplaced,
      encoderQueueDrops: this.windowEncoderQueueDrops,
      encodeTimeMs: this.windowEncodeTimeMs,
      encodedSamples: this.windowEncodedSamples,
    };
  }
}
