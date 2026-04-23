/**
 * AudioController — extracted from BpaneSession.
 *
 * Manages desktop audio output (decode + playback via AudioWorklet),
 * Opus decoding, and microphone capture.
 */

import {
  decodeAudioFramePayload,
} from './audio-frame-decoder.js';
import { AudioPlaybackRuntime } from './audio/audio-playback-runtime.js';
import { MicrophoneRuntime } from './audio/microphone-runtime.js';
import { OpusPlaybackRuntime } from './audio/opus-playback-runtime.js';

export type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

export class AudioController {
  private audioEnabled: boolean;
  private audioPlaybackRuntime: AudioPlaybackRuntime;
  private microphoneRuntime: MicrophoneRuntime;
  private opusPlaybackRuntime: OpusPlaybackRuntime;

  constructor(enabled: boolean, sendFrame: SendFrameFn) {
    this.audioEnabled = enabled;
    this.audioPlaybackRuntime = new AudioPlaybackRuntime();
    this.microphoneRuntime = new MicrophoneRuntime(sendFrame);
    this.opusPlaybackRuntime = new OpusPlaybackRuntime((samples) => {
      this.audioPlaybackRuntime.enqueueSamples(samples);
    });
  }

  static async isMicrophoneSupported(): Promise<boolean> {
    return MicrophoneRuntime.isSupported();
  }

  handleFrame(payload: Uint8Array): void {
    if (!this.audioEnabled) return;
    if (!this.audioPlaybackRuntime.ensureStarted()) {
      return;
    }

    const decodedFrame = decodeAudioFramePayload(payload);
    if (!decodedFrame) {
      return;
    }

    if (decodedFrame.kind === 'opus') {
      try {
        this.opusPlaybackRuntime.decode(decodedFrame.encoded);
      } catch (_) {
        // Opus decode/config failure must not propagate to stream handler
      }
      return;
    }

    if (decodedFrame.samples.length === 0) {
      return;
    }

    this.audioPlaybackRuntime.enqueueSamples(decodedFrame.samples);
  }

  // ── Microphone ─────────────────────────────────────────────────────

  async startMicrophone(): Promise<void> {
    return this.microphoneRuntime.start();
  }

  stopMicrophone(): void {
    this.microphoneRuntime.stop();
  }

  async ensureRecordingStream(): Promise<MediaStream | null> {
    if (!this.audioEnabled) {
      return null;
    }
    return this.audioPlaybackRuntime.ensureRecordingStream();
  }

  destroy(): void {
    this.stopMicrophone();

    this.audioPlaybackRuntime.destroy();
    this.opusPlaybackRuntime.destroy();
  }
}
