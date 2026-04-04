/**
 * AudioWorkletProcessor for low-latency audio playback.
 *
 * Receives interleaved stereo PCM Float32 samples and plays them back
 * using a continuous ring buffer to avoid frame-boundary silence gaps.
 *
 * Ring buffer avoids the alignment problem where 960 stereo samples
 * per 20ms frame is not evenly divisible by the 128-sample render quantum.
 */

const RING_SIZE = 96000; // 1 second of interleaved stereo @ 48kHz
const JITTER_THRESHOLD = 10560; // 110ms of interleaved samples before starting
const MAX_BUFFERED = 19200; // 200ms — drop old samples beyond this
const DROP_TARGET = 9600; // 100ms — target buffer level after drop

declare abstract class AudioWorkletProcessor {
  readonly port: MessagePort;
  constructor();
  abstract process(
    inputs: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>,
  ): boolean;
}

declare function registerProcessor(
  name: string,
  processorCtor: new () => AudioWorkletProcessor,
): void;

class BpaneAudioProcessor extends AudioWorkletProcessor {
  private ring = new Float32Array(RING_SIZE);
  private wPos = 0;
  private rPos = 0;
  private started = false;

  constructor() {
    super();
    this.port.onmessage = (event: MessageEvent) => {
      if (event.data.type === 'audio-data') {
        const samples = new Float32Array(event.data.samples);
        for (let i = 0; i < samples.length; i++) {
          this.ring[this.wPos] = samples[i];
          this.wPos = (this.wPos + 1) % RING_SIZE;
        }
      }
    };
  }

  process(
    _inputs: Float32Array[][],
    outputs: Float32Array[][],
    _parameters: Record<string, Float32Array>,
  ): boolean {
    const output = outputs[0];
    if (!output || output.length < 2) return true;

    const left = output[0];
    const right = output[1];
    const n = left.length; // 128 (render quantum)

    let avail = this.wPos - this.rPos;
    if (avail < 0) avail += RING_SIZE;

    // Wait for jitter buffer to fill before starting playback
    if (!this.started) {
      if (avail < JITTER_THRESHOLD) {
        left.fill(0);
        right.fill(0);
        return true;
      }
      this.started = true;
    }

    // If buffer drains completely, pause until refilled
    if (avail < n * 2) {
      left.fill(0);
      right.fill(0);
      this.started = false;
      return true;
    }

    // Drop old samples if buffer grows too large (clock drift protection)
    if (avail > MAX_BUFFERED) {
      const drop = avail - DROP_TARGET;
      this.rPos = (this.rPos + drop) % RING_SIZE;
    }

    // Deinterleave: ring contains [L0, R0, L1, R1, ...]
    for (let i = 0; i < n; i++) {
      left[i] = this.ring[this.rPos];
      this.rPos = (this.rPos + 1) % RING_SIZE;
      right[i] = this.ring[this.rPos];
      this.rPos = (this.rPos + 1) % RING_SIZE;
    }

    return true;
  }
}

registerProcessor('bpane-audio-processor', BpaneAudioProcessor);

export {};
