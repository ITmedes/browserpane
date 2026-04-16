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

export type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

export class AudioController {
  private audioContext: AudioContext | null = null;
  private audioWorkletNode: AudioWorkletNode | null = null;
  private audioInitialized = false;
  private audioEnabled: boolean;
  private opusDecoder: AudioDecoder | null = null;
  private opusTimestamp = 0;
  // Reusable buffers for Opus decode to avoid per-frame GC pressure.
  // 960 samples/channel x 2 channels = 1920 interleaved samples per 20ms frame.
  private opusPlane = new Float32Array(960);
  private opusInterleaved = new Float32Array(1920);
  private microphoneRuntime: MicrophoneRuntime;

  constructor(enabled: boolean, sendFrame: SendFrameFn) {
    this.audioEnabled = enabled;
    this.microphoneRuntime = new MicrophoneRuntime(sendFrame);
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
        this.decodeOpus(decodedFrame.encoded);
      } catch (_) {
        // Opus decode/config failure must not propagate to stream handler
      }
      return;
    }

    if (decodedFrame.samples.length === 0) {
      return;
    }

    this.audioWorkletNode.port.postMessage(
      { type: 'audio-data', samples: decodedFrame.samples.buffer },
      [decodedFrame.samples.buffer],
    );
  }

  private decodeOpus(data: Uint8Array): void {
    if (!this.audioWorkletNode) return;
    const worklet = this.audioWorkletNode;

    if (!this.opusDecoder) {
      const plane = this.opusPlane;
      const interleaved = this.opusInterleaved;

      this.opusDecoder = new AudioDecoder({
        output: (frame: AudioData) => {
          const n = frame.numberOfFrames;
          const ch = frame.numberOfChannels;
          const fmt = (frame as any).format as string | undefined;
          if (ch === 0 || n === 0) { frame.close(); return; }

          const interleavedNeeded = n * 2;
          let il: Float32Array;

          if (fmt === 'f32' || fmt === 'f32-interleaved') {
            // Already interleaved f32 — copy directly
            il = new Float32Array(n * ch);
            frame.copyTo(il, { planeIndex: 0 });
            if (ch === 1) {
              // Expand mono to stereo
              const stereo = new Float32Array(interleavedNeeded);
              for (let i = 0; i < n; i++) {
                stereo[i * 2] = il[i];
                stereo[i * 2 + 1] = il[i];
              }
              il = stereo;
            }
          } else {
            // f32-planar (default for Opus in Chrome)
            const planeNeeded = n;
            let p = plane.length >= planeNeeded ? plane : new Float32Array(planeNeeded);
            il = interleaved.length >= interleavedNeeded ? interleaved : new Float32Array(interleavedNeeded);

            if (ch === 1) {
              frame.copyTo(p, { planeIndex: 0, format: 'f32-planar' } as any);
              for (let i = 0; i < n; i++) {
                il[i * 2] = p[i];
                il[i * 2 + 1] = p[i];
              }
            } else {
              frame.copyTo(p, { planeIndex: 0, format: 'f32-planar' } as any);
              for (let i = 0; i < n; i++) il[i * 2] = p[i];
              frame.copyTo(p, { planeIndex: 1, format: 'f32-planar' } as any);
              for (let i = 0; i < n; i++) il[i * 2 + 1] = p[i];
            }
          }
          frame.close();

          // Transfer a copy to the worklet (the buffer is transferred, not shared)
          const out = il.slice(0, interleavedNeeded);
          worklet.port.postMessage(
            { type: 'audio-data', samples: out.buffer },
            [out.buffer],
          );
        },
        error: (e: DOMException) => {
          console.error('[bpane] Opus AudioDecoder error:', e.message);
          this.opusDecoder = null;
        },
      });
      this.opusDecoder.configure({
        codec: 'opus',
        numberOfChannels: 2,
        sampleRate: 48000,
      });
      this.opusTimestamp = 0;
    }

    // Feed Opus packet to the decoder — copy since subarray view may be invalidated
    const opusData = data.slice(0);
    this.opusDecoder.decode(new EncodedAudioChunk({
      type: 'key', // Opus frames are independently decodable
      timestamp: this.opusTimestamp,
      data: opusData,
    }));
    this.opusTimestamp += 20_000; // 20ms in microseconds
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
    if (this.opusDecoder) {
      try { this.opusDecoder.close(); } catch (_) { /* ignore */ }
      this.opusDecoder = null;
      this.opusTimestamp = 0;
    }
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
    this.audioInitialized = false;
  }
}
