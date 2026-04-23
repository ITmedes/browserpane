const PLAYBACK_SAMPLE_RATE = 48000;
const PLAYBACK_WORKLET_NAME = 'bpane-audio-processor';

function resolvePlaybackWorkletModuleUrl(): string {
  const moduleBaseUrl = import.meta.url;
  return new URL('./audio-worklet.js', moduleBaseUrl).href;
}

export class AudioPlaybackRuntime {
  private audioContext: AudioContext | null = null;
  private audioWorkletNode: AudioWorkletNode | null = null;
  private recordingDestination: MediaStreamAudioDestinationNode | null = null;
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

  async ensureRecordingStream(): Promise<MediaStream | null> {
    if (!this.audioWorkletNode && !this.starting) {
      this.starting = this.start();
    }
    if (this.starting) {
      await this.starting;
    }
    return this.recordingDestination?.stream ?? null;
  }

  destroy(): void {
    this.detachResumeHandler();

    if (this.audioWorkletNode) {
      this.audioWorkletNode.disconnect();
      this.audioWorkletNode = null;
    }
    this.recordingDestination = null;
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
      await audioContext.audioWorklet.addModule(resolvePlaybackWorkletModuleUrl());

      if (this.audioContext !== audioContext) {
        return;
      }

      const audioWorkletNode = new AudioWorkletNode(
        audioContext,
        PLAYBACK_WORKLET_NAME,
        { outputChannelCount: [2] },
      );
      audioWorkletNode.connect(audioContext.destination);
      if (typeof audioContext.createMediaStreamDestination === 'function') {
        this.recordingDestination = audioContext.createMediaStreamDestination();
        audioWorkletNode.connect(this.recordingDestination);
      }
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
