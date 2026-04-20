import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type RegisteredProcessorCtor = new () => {
  port: {
    onmessage: ((event: MessageEvent) => void) | null;
  };
  process(
    inputs: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>,
  ): boolean;
};

let registeredName: string | null = null;
let registeredCtor: RegisteredProcessorCtor | null = null;

class MockAudioWorkletProcessor {
  readonly port = {
    onmessage: null as ((event: MessageEvent) => void) | null,
  };
}

async function loadWorklet(): Promise<RegisteredProcessorCtor> {
  registeredName = null;
  registeredCtor = null;
  vi.resetModules();
  vi.stubGlobal('AudioWorkletProcessor', MockAudioWorkletProcessor);
  vi.stubGlobal('registerProcessor', vi.fn((name: string, ctor: RegisteredProcessorCtor) => {
    registeredName = name;
    registeredCtor = ctor;
  }));
  await import('../audio/audio-worklet.js');
  expect(registeredName).toBe('bpane-audio-processor');
  expect(registeredCtor).not.toBeNull();
  return registeredCtor!;
}

function enqueueSamples(processor: InstanceType<RegisteredProcessorCtor>, samples: Float32Array): void {
  processor.port.onmessage?.({
    data: {
      type: 'audio-data',
      samples: samples.buffer.slice(0),
    },
  } as MessageEvent);
}

function makeStereoOutputs(frameSize = 128): Float32Array[][] {
  return [[new Float32Array(frameSize), new Float32Array(frameSize)]];
}

describe('audio-worklet', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('registers the Bpane audio processor and waits for the jitter buffer', async () => {
    const Processor = await loadWorklet();
    const processor = new Processor();
    const outputs = makeStereoOutputs();

    enqueueSamples(processor, new Float32Array(1024));
    expect(processor.process([], outputs, {})).toBe(true);
    expect(Array.from(outputs[0][0])).toEqual(new Array(128).fill(0));
    expect(Array.from(outputs[0][1])).toEqual(new Array(128).fill(0));
  });

  it('deinterleaves stereo samples once the buffer is primed', async () => {
    const Processor = await loadWorklet();
    const processor = new Processor();
    const samples = new Float32Array(11_000);
    for (let i = 0; i < samples.length / 2; i++) {
      samples[i * 2] = i + 0.25;
      samples[i * 2 + 1] = -(i + 0.25);
    }

    enqueueSamples(processor, samples);
    const outputs = makeStereoOutputs();
    processor.process([], outputs, {});

    expect(outputs[0][0][0]).toBeCloseTo(0.25);
    expect(outputs[0][1][0]).toBeCloseTo(-0.25);
    expect(outputs[0][0][127]).toBeCloseTo(127.25);
    expect(outputs[0][1][127]).toBeCloseTo(-127.25);
  });

  it('drops stale buffered samples when latency grows too large', async () => {
    const Processor = await loadWorklet();
    const processor = new Processor();
    const samples = new Float32Array(20_000);
    for (let i = 0; i < samples.length; i++) samples[i] = i;

    enqueueSamples(processor, samples);
    const outputs = makeStereoOutputs();
    processor.process([], outputs, {});

    expect(outputs[0][0][0]).toBe(10_400);
    expect(outputs[0][1][0]).toBe(10_401);
    expect(outputs[0][0][1]).toBe(10_402);
    expect(outputs[0][1][1]).toBe(10_403);
  });

  it('returns to silence after underrun until enough audio is buffered again', async () => {
    const Processor = await loadWorklet();
    const processor = new Processor();

    enqueueSamples(processor, new Float32Array(11_000).fill(1));
    for (let i = 0; i < 42; i++) {
      processor.process([], makeStereoOutputs(), {});
    }

    const underrun = makeStereoOutputs();
    processor.process([], underrun, {});
    expect(Array.from(underrun[0][0])).toEqual(new Array(128).fill(0));
    expect(Array.from(underrun[0][1])).toEqual(new Array(128).fill(0));

    enqueueSamples(processor, new Float32Array(256).fill(0.5));
    const stillBuffering = makeStereoOutputs();
    processor.process([], stillBuffering, {});
    expect(Array.from(stillBuffering[0][0])).toEqual(new Array(128).fill(0));
    expect(Array.from(stillBuffering[0][1])).toEqual(new Array(128).fill(0));
  });
});
