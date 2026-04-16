/**
 * AudioController — extracted from BpaneSession.
 *
 * Manages desktop audio output (decode + playback via AudioWorklet),
 * Opus decoding, and microphone capture.
 */

import {
  decodeAudioFramePayload,
} from './audio-frame-decoder.js';
import { MicrophoneRuntime } from './audio/microphone-runtime.js';
import { OpusPlaybackRuntime } from './audio/opus-playback-runtime.js';

export type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

export class AudioController {
  private audioContext: AudioContext | null = null;
  private audioWorkletNode: AudioWorkletNode | null = null;
  private audioInitialized = false;
  private audioEnabled: boolean;
  private microphoneRuntime: MicrophoneRuntime;
  private opusPlaybackRuntime: OpusPlaybackRuntime;

  constructor(enabled: boolean, sendFrame: SendFrameFn) {
    this.audioEnabled = enabled;
    this.microphoneRuntime = new MicrophoneRuntime(sendFrame);
    this.opusPlaybackRuntime = new OpusPlaybackRuntime((samples) => {
      this.postSamplesToWorklet(samples);
    });
  }

  static async isMicrophoneSupported(): Promise<boolean> {
    return MicrophoneRuntime.isSupported();
  }

  handleFrame(payload: Uint8Array): void {
    if (!this.audioEnabled) return;
    if (!this.audioInitialized) {
      this.initAudio();
      return;
    }
    if (!this.audioWorkletNode) return;

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

    this.postSamplesToWorklet(decodedFrame.samples);
  }

  private initAudio(): void {
    if (this.audioInitialized) return;
    this.audioInitialized = true;

    const start = async () => {
      try {
        this.audioContext = new AudioContext({ sampleRate: 48000 });

        const workletCode = `
class BpaneAudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.ring = new Float32Array(96000);
    this.wPos = 0;
    this.rPos = 0;
    this.started = false;
    this.port.onmessage = (e) => {
      if (e.data.type === 'audio-data') {
        const samples = new Float32Array(e.data.samples);
        for (let i = 0; i < samples.length; i++) {
          this.ring[this.wPos] = samples[i];
          this.wPos = (this.wPos + 1) % 96000;
        }
      }
    };
  }
  process(inputs, outputs) {
    const output = outputs[0];
    if (!output || output.length < 2) return true;
    const left = output[0];
    const right = output[1];
    const n = left.length;
    let avail = this.wPos - this.rPos;
    if (avail < 0) avail += 96000;
    if (!this.started) {
      if (avail < 10560) { left.fill(0); right.fill(0); return true; }
      this.started = true;
    }
    if (avail < n * 2) {
      left.fill(0); right.fill(0);
      this.started = false;
      return true;
    }
    if (avail > 19200) {
      const drop = avail - 9600;
      this.rPos = (this.rPos + drop) % 96000;
    }
    for (let i = 0; i < n; i++) {
      left[i] = this.ring[this.rPos];
      this.rPos = (this.rPos + 1) % 96000;
      right[i] = this.ring[this.rPos];
      this.rPos = (this.rPos + 1) % 96000;
    }
    return true;
  }
}
registerProcessor('bpane-audio-processor', BpaneAudioProcessor);
`;
        const blob = new Blob([workletCode], { type: 'application/javascript' });
        const url = URL.createObjectURL(blob);
        await this.audioContext.audioWorklet.addModule(url);
        URL.revokeObjectURL(url);

        this.audioWorkletNode = new AudioWorkletNode(
          this.audioContext,
          'bpane-audio-processor',
          { outputChannelCount: [2] },
        );
        this.audioWorkletNode.connect(this.audioContext.destination);

        if (this.audioContext.state === 'suspended') {
          const resume = () => {
            this.audioContext?.resume();
            document.removeEventListener('click', resume);
            document.removeEventListener('keydown', resume);
          };
          document.addEventListener('click', resume);
          document.addEventListener('keydown', resume);
        }
      } catch (e) {
        console.error('[bpane] audio init failed:', e);
        this.audioInitialized = false;
      }
    };
    start();
  }

  private postSamplesToWorklet(samples: Float32Array): void {
    if (!this.audioWorkletNode || samples.length === 0) {
      return;
    }

    this.audioWorkletNode.port.postMessage(
      { type: 'audio-data', samples: samples.buffer },
      [samples.buffer],
    );
  }

  // ── Microphone ─────────────────────────────────────────────────────

  async startMicrophone(): Promise<void> {
    return this.microphoneRuntime.start();
  }

  stopMicrophone(): void {
    this.microphoneRuntime.stop();
  }

  destroy(): void {
    this.stopMicrophone();

    if (this.audioWorkletNode) {
      this.audioWorkletNode.disconnect();
      this.audioWorkletNode = null;
    }
    this.opusPlaybackRuntime.destroy();
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
    this.audioInitialized = false;
  }
}
