import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AudioPlaybackRuntime } from '../audio/audio-playback-runtime.js';

class MockPort {
  readonly postMessage = vi.fn();
}

class MockAudioWorkletNode {
  static instances: MockAudioWorkletNode[] = [];

  readonly port = new MockPort();
  readonly connect = vi.fn();
  readonly disconnect = vi.fn();

  constructor(
    _context: BaseAudioContext,
    _name: string,
    _options?: AudioWorkletNodeOptions,
  ) {
    MockAudioWorkletNode.instances.push(this);
  }
}

class MockAudioContext {
  static instances: MockAudioContext[] = [];
  static nextAddModuleError: Error | null = null;
  static initialState: AudioContextState = 'running';

  readonly audioWorklet = {
    addModule: vi.fn(async () => {
      if (MockAudioContext.nextAddModuleError) {
        const error = MockAudioContext.nextAddModuleError;
        MockAudioContext.nextAddModuleError = null;
        throw error;
      }
    }),
  };
  readonly destination = {} as AudioDestinationNode;
  readonly resume = vi.fn(async () => {});
  readonly close = vi.fn(async () => {});
  state: AudioContextState;

  constructor(_options?: AudioContextOptions) {
    this.state = MockAudioContext.initialState;
    MockAudioContext.instances.push(this);
  }
}

async function flushAsync(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('AudioPlaybackRuntime', () => {
  beforeEach(() => {
    MockAudioContext.instances = [];
    MockAudioContext.nextAddModuleError = null;
    MockAudioContext.initialState = 'running';
    MockAudioWorkletNode.instances = [];

    vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.stubGlobal('AudioContext', MockAudioContext);
    vi.stubGlobal('AudioWorkletNode', MockAudioWorkletNode);
    Object.defineProperty(URL, 'createObjectURL', {
      configurable: true,
      writable: true,
      value: vi.fn(() => 'blob:mock-audio'),
    });
    Object.defineProperty(URL, 'revokeObjectURL', {
      configurable: true,
      writable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('initializes playback and posts samples once the worklet is ready', async () => {
    const runtime = new AudioPlaybackRuntime();

    expect(runtime.ensureStarted()).toBe(false);
    await flushAsync();

    expect(MockAudioContext.instances).toHaveLength(1);
    expect(MockAudioContext.instances[0].audioWorklet.addModule).toHaveBeenCalledWith('blob:mock-audio');
    expect(MockAudioWorkletNode.instances).toHaveLength(1);
    expect(MockAudioWorkletNode.instances[0].connect).toHaveBeenCalledWith(MockAudioContext.instances[0].destination);
    expect(runtime.ensureStarted()).toBe(true);

    runtime.enqueueSamples(new Float32Array([0.25, -0.25, 0.5, -0.5]));

    expect(MockAudioWorkletNode.instances[0].port.postMessage).toHaveBeenCalledTimes(1);
    const [message] = MockAudioWorkletNode.instances[0].port.postMessage.mock.calls[0];
    expect(Array.from(new Float32Array(message.samples as ArrayBuffer))).toEqual([0.25, -0.25, 0.5, -0.5]);
  });

  it('resumes suspended playback after the next user gesture', async () => {
    const runtime = new AudioPlaybackRuntime();
    MockAudioContext.initialState = 'suspended';
    const addEventListenerSpy = vi.spyOn(document, 'addEventListener');
    const removeEventListenerSpy = vi.spyOn(document, 'removeEventListener');

    runtime.ensureStarted();
    await flushAsync();

    const clickResume = addEventListenerSpy.mock.calls.find(([eventType]) => eventType === 'click')?.[1] as EventListener;
    expect(clickResume).toBeTypeOf('function');

    clickResume(new Event('click'));

    expect(MockAudioContext.instances[0].resume).toHaveBeenCalledTimes(1);
    expect(removeEventListenerSpy).toHaveBeenCalledWith('click', clickResume);
    expect(removeEventListenerSpy).toHaveBeenCalledWith('keydown', clickResume);
  });

  it('ignores empty samples and tears down worklet resources on destroy', async () => {
    const runtime = new AudioPlaybackRuntime();

    runtime.ensureStarted();
    await flushAsync();

    runtime.enqueueSamples(new Float32Array());
    expect(MockAudioWorkletNode.instances[0].port.postMessage).not.toHaveBeenCalled();

    runtime.destroy();

    expect(MockAudioWorkletNode.instances[0].disconnect).toHaveBeenCalledTimes(1);
    expect(MockAudioContext.instances[0].close).toHaveBeenCalledTimes(1);
    expect(runtime.ensureStarted()).toBe(false);
  });

  it('logs startup failures, resets state, and retries on the next frame', async () => {
    const runtime = new AudioPlaybackRuntime();
    MockAudioContext.nextAddModuleError = new Error('worklet failed');

    expect(runtime.ensureStarted()).toBe(false);
    await flushAsync();

    expect(console.error).toHaveBeenCalledWith('[bpane] audio init failed:', expect.any(Error));
    expect(MockAudioWorkletNode.instances).toHaveLength(0);

    expect(runtime.ensureStarted()).toBe(false);
    await flushAsync();

    expect(MockAudioWorkletNode.instances).toHaveLength(1);
  });
});
