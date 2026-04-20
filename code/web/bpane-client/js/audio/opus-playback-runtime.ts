const OPUS_SAMPLE_RATE = 48000;
const OPUS_CHANNELS = 2;
const OPUS_FRAME_DURATION_US = 20_000;

type OutputSamplesFn = (samples: Float32Array) => void;

export class OpusPlaybackRuntime {
  private opusDecoder: AudioDecoder | null = null;
  private opusTimestamp = 0;
  // Reusable buffers for Opus decode to avoid per-frame GC pressure.
  // 960 samples/channel x 2 channels = 1920 interleaved samples per 20ms frame.
  private opusPlane = new Float32Array(960);
  private opusInterleaved = new Float32Array(1920);
  private readonly onSamples: OutputSamplesFn;

  constructor(onSamples: OutputSamplesFn) {
    this.onSamples = onSamples;
  }

  decode(packet: Uint8Array): void {
    if (!this.opusDecoder) {
      this.initializeDecoder();
    }

    const opusData = packet.slice(0);
    this.opusDecoder!.decode(new EncodedAudioChunk({
      type: 'key',
      timestamp: this.opusTimestamp,
      data: opusData,
    }));
    this.opusTimestamp += OPUS_FRAME_DURATION_US;
  }

  destroy(): void {
    if (!this.opusDecoder) {
      this.opusTimestamp = 0;
      return;
    }

    try {
      this.opusDecoder.close();
    } catch (_) {
      // Ignore decoder close failures during teardown.
    }
    this.opusDecoder = null;
    this.opusTimestamp = 0;
  }

  private initializeDecoder(): void {
    const plane = this.opusPlane;
    const interleaved = this.opusInterleaved;

    this.opusDecoder = new AudioDecoder({
      output: (frame: AudioData) => {
        const numberOfFrames = frame.numberOfFrames;
        const numberOfChannels = frame.numberOfChannels;
        const format = (frame as { format?: string }).format;

        if (numberOfChannels === 0 || numberOfFrames === 0) {
          frame.close();
          return;
        }

        const interleavedNeeded = numberOfFrames * OPUS_CHANNELS;
        let output: Float32Array;

        if (format === 'f32' || format === 'f32-interleaved') {
          output = new Float32Array(numberOfFrames * numberOfChannels);
          frame.copyTo(output, { planeIndex: 0 });
          if (numberOfChannels === 1) {
            const stereo = new Float32Array(interleavedNeeded);
            for (let i = 0; i < numberOfFrames; i += 1) {
              stereo[i * 2] = output[i];
              stereo[i * 2 + 1] = output[i];
            }
            output = stereo;
          }
        } else {
          let planeBuffer = plane.length >= numberOfFrames ? plane : new Float32Array(numberOfFrames);
          output = interleaved.length >= interleavedNeeded
            ? interleaved
            : new Float32Array(interleavedNeeded);

          if (numberOfChannels === 1) {
            frame.copyTo(planeBuffer, { planeIndex: 0, format: 'f32-planar' } as AudioDataCopyToOptions);
            for (let i = 0; i < numberOfFrames; i += 1) {
              output[i * 2] = planeBuffer[i];
              output[i * 2 + 1] = planeBuffer[i];
            }
          } else {
            frame.copyTo(planeBuffer, { planeIndex: 0, format: 'f32-planar' } as AudioDataCopyToOptions);
            for (let i = 0; i < numberOfFrames; i += 1) {
              output[i * 2] = planeBuffer[i];
            }
            frame.copyTo(planeBuffer, { planeIndex: 1, format: 'f32-planar' } as AudioDataCopyToOptions);
            for (let i = 0; i < numberOfFrames; i += 1) {
              output[i * 2 + 1] = planeBuffer[i];
            }
          }
        }

        frame.close();
        this.onSamples(output.slice(0, interleavedNeeded));
      },
      error: (error: DOMException) => {
        console.error('[bpane] Opus AudioDecoder error:', error.message);
        this.opusDecoder = null;
      },
    });
    this.opusDecoder.configure({
      codec: 'opus',
      numberOfChannels: OPUS_CHANNELS,
      sampleRate: OPUS_SAMPLE_RATE,
    });
    this.opusTimestamp = 0;
  }
}
