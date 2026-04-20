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
const MAX_BUFFERED = 19200; // 200ms - drop old samples beyond this
const DROP_TARGET = 9600; // 100ms - target buffer level after drop

class BpaneAudioProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.ring = new Float32Array(RING_SIZE);
    this.wPos = 0;
    this.rPos = 0;
    this.started = false;

    this.port.onmessage = (event) => {
      if (event.data.type === 'audio-data') {
        const samples = new Float32Array(event.data.samples);
        for (let i = 0; i < samples.length; i += 1) {
          this.ring[this.wPos] = samples[i];
          this.wPos = (this.wPos + 1) % RING_SIZE;
        }
      }
    };
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (!output || output.length < 2) return true;

    const left = output[0];
    const right = output[1];
    const frameSize = left.length;

    let available = this.wPos - this.rPos;
    if (available < 0) available += RING_SIZE;

    if (!this.started) {
      if (available < JITTER_THRESHOLD) {
        left.fill(0);
        right.fill(0);
        return true;
      }
      this.started = true;
    }

    if (available < frameSize * 2) {
      left.fill(0);
      right.fill(0);
      this.started = false;
      return true;
    }

    if (available > MAX_BUFFERED) {
      const drop = available - DROP_TARGET;
      this.rPos = (this.rPos + drop) % RING_SIZE;
    }

    for (let i = 0; i < frameSize; i += 1) {
      left[i] = this.ring[this.rPos];
      this.rPos = (this.rPos + 1) % RING_SIZE;
      right[i] = this.ring[this.rPos];
      this.rPos = (this.rPos + 1) % RING_SIZE;
    }

    return true;
  }
}

registerProcessor('bpane-audio-processor', BpaneAudioProcessor);
