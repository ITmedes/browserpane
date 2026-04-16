const PLAYBACK_SAMPLE_RATE = 48000;
const PLAYBACK_WORKLET_NAME = 'bpane-audio-processor';

function buildAudioPlaybackWorkletCode(): string {
  return `
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
registerProcessor('${PLAYBACK_WORKLET_NAME}', BpaneAudioProcessor);
`;
}

export class AudioPlaybackRuntime {
  private audioContext: AudioContext | null = null;
  private audioWorkletNode: AudioWorkletNode | null = null;
  private starting: Promise<void> | null = null;
  private resumeHandler: (() => void) | null = null;

  ensureStarted(): boolean {
    if (this.audioWorkletNode) {
      return true;
    }
    if (!this.starting) {
      this.starting = this.start();
    }
    return false;
  }

  enqueueSamples(samples: Float32Array): void {
    if (!this.audioWorkletNode || samples.length === 0) {
      return;
    }

    this.audioWorkletNode.port.postMessage(
      { type: 'audio-data', samples: samples.buffer },
      [samples.buffer],
    );
  }

  destroy(): void {
    this.detachResumeHandler();

    if (this.audioWorkletNode) {
      this.audioWorkletNode.disconnect();
      this.audioWorkletNode = null;
    }
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
    this.starting = null;
  }

  private async start(): Promise<void> {
    const audioContext = new AudioContext({ sampleRate: PLAYBACK_SAMPLE_RATE });
    this.audioContext = audioContext;

    try {
      const blob = new Blob([buildAudioPlaybackWorkletCode()], {
        type: 'application/javascript',
      });
      const url = URL.createObjectURL(blob);
      try {
        await audioContext.audioWorklet.addModule(url);
      } finally {
        URL.revokeObjectURL(url);
      }

      if (this.audioContext !== audioContext) {
        return;
      }

      const audioWorkletNode = new AudioWorkletNode(
        audioContext,
        PLAYBACK_WORKLET_NAME,
        { outputChannelCount: [2] },
      );
      audioWorkletNode.connect(audioContext.destination);
      this.audioWorkletNode = audioWorkletNode;

      if (audioContext.state === 'suspended') {
        this.attachResumeHandler();
      }
    } catch (error) {
      console.error('[bpane] audio init failed:', error);
      if (this.audioContext === audioContext) {
        this.audioContext = null;
      }
      try {
        await audioContext.close();
      } catch (_) {
        // Ignore context close failures during init recovery.
      }
    } finally {
      this.starting = null;
    }
  }

  private attachResumeHandler(): void {
    if (this.resumeHandler) {
      return;
    }

    this.resumeHandler = () => {
      this.audioContext?.resume();
      this.detachResumeHandler();
    };

    document.addEventListener('click', this.resumeHandler);
    document.addEventListener('keydown', this.resumeHandler);
  }

  private detachResumeHandler(): void {
    if (!this.resumeHandler) {
      return;
    }

    document.removeEventListener('click', this.resumeHandler);
    document.removeEventListener('keydown', this.resumeHandler);
    this.resumeHandler = null;
  }
}
