import { AUDIO_CODEC_OPUS } from '../audio-frame-decoder.js';
import { CH_AUDIO_IN } from '../protocol.js';

const MIC_SAMPLE_RATE = 48000;
const MIC_CHANNELS = 1;
const MIC_FRAME_DURATION_US = 20_000;
const MIC_SAMPLES_PER_FRAME = (MIC_SAMPLE_RATE / 1000) * 20;
const MIC_OPUS_BITRATE_BPS = 32_000;
const MIC_OPUS_CONFIG: AudioEncoderConfig = {
  codec: 'opus',
  sampleRate: MIC_SAMPLE_RATE,
  numberOfChannels: MIC_CHANNELS,
  bitrate: MIC_OPUS_BITRATE_BPS,
  opus: {
    format: 'opus',
    frameDuration: MIC_FRAME_DURATION_US,
  },
};

const MIC_CAPTURE_WORKLET_NAME = 'mic-capture';

type SendFrameFn = (channelId: number, payload: Uint8Array) => void;

function buildMicrophoneCaptureWorkletCode(): string {
  return `
class MicCaptureProcessor extends AudioWorkletProcessor {
  constructor() { super(); this.buffer = new Float32Array(${MIC_SAMPLES_PER_FRAME}); this.pos = 0; }
  process(inputs) {
    const input = inputs[0];
    if (!input || !input[0]) return true;
    const samples = input[0];
    for (let i = 0; i < samples.length; i++) {
      this.buffer[this.pos++] = samples[i];
      if (this.pos >= ${MIC_SAMPLES_PER_FRAME}) {
        const pcm = new Int16Array(${MIC_SAMPLES_PER_FRAME});
        for (let j = 0; j < ${MIC_SAMPLES_PER_FRAME}; j++) {
          pcm[j] = Math.max(-32768, Math.min(32767, Math.round(this.buffer[j] * 32767)));
        }
        this.port.postMessage({ type: 'pcm', data: pcm.buffer }, [pcm.buffer]);
        this.buffer = new Float32Array(${MIC_SAMPLES_PER_FRAME});
        this.pos = 0;
      }
    }
    return true;
  }
}
registerProcessor('${MIC_CAPTURE_WORKLET_NAME}', MicCaptureProcessor);
`;
}

export class MicrophoneRuntime {
  private micStream: MediaStream | null = null;
  private micContext: AudioContext | null = null;
  private micWorklet: AudioWorkletNode | null = null;
  private micEncoder: AudioEncoder | null = null;
  private micActive = false;
  private micSeq = 0;
  private micTimestampUs = 0;
  private readonly sendFrame: SendFrameFn;

  constructor(sendFrame: SendFrameFn) {
    this.sendFrame = sendFrame;
  }

  static async isSupported(): Promise<boolean> {
    if (typeof AudioEncoder === 'undefined' || typeof AudioData === 'undefined') {
      return false;
    }
    try {
      const support = await AudioEncoder.isConfigSupported(MIC_OPUS_CONFIG);
      return !!support.supported;
    } catch {
      return false;
    }
  }

  async start(): Promise<void> {
    if (this.micActive) return;

    try {
      const supported = await MicrophoneRuntime.isSupported();
      if (!supported) {
        throw new Error('browser Opus microphone encoding is unavailable');
      }

      this.micStream = await navigator.mediaDevices.getUserMedia({
        audio: { sampleRate: MIC_SAMPLE_RATE, channelCount: MIC_CHANNELS, echoCancellation: true },
      });
      this.micContext = new AudioContext({ sampleRate: MIC_SAMPLE_RATE });
      this.micEncoder = new AudioEncoder({
        output: (chunk: EncodedAudioChunk) => {
          const opusPacket = new Uint8Array(chunk.byteLength);
          chunk.copyTo(opusPacket);
          this.sendMicFrame(opusPacket);
        },
        error: (e: DOMException) => {
          console.error('[bpane] microphone encoder error:', e.message);
          this.micEncoder = null;
        },
      });
      this.micEncoder.configure(MIC_OPUS_CONFIG);

      const blob = new Blob([buildMicrophoneCaptureWorkletCode()], {
        type: 'application/javascript',
      });
      const url = URL.createObjectURL(blob);
      try {
        await this.micContext.audioWorklet.addModule(url);
      } finally {
        URL.revokeObjectURL(url);
      }

      this.micWorklet = new AudioWorkletNode(this.micContext, MIC_CAPTURE_WORKLET_NAME);
      this.micWorklet.port.onmessage = (event: MessageEvent) => {
        if (event.data.type !== 'pcm' || !this.micEncoder) {
          return;
        }

        const pcmBuffer = event.data.data as ArrayBuffer;
        if (!(pcmBuffer instanceof ArrayBuffer)) {
          return;
        }

        if (this.micEncoder.encodeQueueSize > 3) {
          this.micTimestampUs += MIC_FRAME_DURATION_US;
          return;
        }

        const audioData = new AudioData({
          format: 's16',
          sampleRate: MIC_SAMPLE_RATE,
          numberOfChannels: MIC_CHANNELS,
          numberOfFrames: pcmBuffer.byteLength / 2,
          timestamp: this.micTimestampUs,
          data: pcmBuffer,
        });
        this.micTimestampUs += MIC_FRAME_DURATION_US;
        try {
          this.micEncoder.encode(audioData);
        } finally {
          audioData.close();
        }
      };

      const source = this.micContext.createMediaStreamSource(this.micStream);
      source.connect(this.micWorklet);
      this.micActive = true;
    } catch (error) {
      this.cleanup();
      console.error('[bpane] microphone error:', error);
    }
  }

  stop(): void {
    if (!this.micActive && !this.micWorklet && !this.micEncoder && !this.micContext && !this.micStream) {
      return;
    }

    this.cleanup();
  }

  private cleanup(): void {
    this.micActive = false;

    if (this.micWorklet) {
      this.micWorklet.disconnect();
      this.micWorklet = null;
    }
    if (this.micEncoder) {
      try {
        this.micEncoder.close();
      } catch (_) {
        // Ignore encoder close failures during teardown.
      }
      this.micEncoder = null;
    }
    if (this.micContext) {
      this.micContext.close();
      this.micContext = null;
    }
    if (this.micStream) {
      this.micStream.getTracks().forEach((track) => track.stop());
      this.micStream = null;
    }
  }

  private sendMicFrame(opusPacket: Uint8Array): void {
    this.micSeq += 1;
    const timestampUs = this.micSeq * MIC_FRAME_DURATION_US;
    const audioPayload = new Uint8Array(5 + opusPacket.length);
    audioPayload[0] = 0x57;
    audioPayload[1] = 0x52;
    audioPayload[2] = 0x41;
    audioPayload[3] = 0x31;
    audioPayload[4] = AUDIO_CODEC_OPUS;
    audioPayload.set(opusPacket, 5);

    const header = new Uint8Array(16);
    const view = new DataView(header.buffer);
    view.setUint32(0, this.micSeq, true);
    view.setUint32(4, timestampUs & 0xFFFFFFFF, true);
    view.setUint32(8, Math.floor(timestampUs / 0x100000000), true);
    view.setUint32(12, audioPayload.length, true);

    const payload = new Uint8Array(16 + audioPayload.length);
    payload.set(header, 0);
    payload.set(audioPayload, 16);
    this.sendFrame(CH_AUDIO_IN, payload);
  }
}
