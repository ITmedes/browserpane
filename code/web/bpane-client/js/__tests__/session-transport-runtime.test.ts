import { afterEach, describe, expect, it, vi } from 'vitest';

import { SessionTransportRuntime } from '../session-transport-runtime.js';
import { UnsupportedFeatureError } from '../shared/errors.js';

class MockReadableStream<T> {
  private reader = new MockReader<T>();

  getReader(): MockReader<T> {
    return this.reader;
  }

  pushValue(value: T): void {
    this.reader.pushValue(value);
  }

  end(): void {
    this.reader.end();
  }

  fail(error: unknown): void {
    this.reader.fail(error);
  }
}

class MockReader<T> {
  private queue: Array<{ value: T | undefined; done: boolean }> = [];
  private resolvers: Array<{
    resolve: (result: { value: T | undefined; done: boolean }) => void;
    reject: (error: unknown) => void;
  }> = [];
  private ended = false;
  private failure: unknown = null;

  pushValue(value: T): void {
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!.resolve({ value, done: false });
      return;
    }
    this.queue.push({ value, done: false });
  }

  end(): void {
    this.ended = true;
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!.resolve({ value: undefined, done: true });
      return;
    }
    this.queue.push({ value: undefined, done: true });
  }

  fail(error: unknown): void {
    this.failure = error;
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!.reject(error);
    }
  }

  async read(): Promise<{ value: T | undefined; done: boolean }> {
    if (this.failure !== null) {
      const error = this.failure;
      this.failure = null;
      throw error;
    }
    if (this.queue.length > 0) {
      return this.queue.shift()!;
    }
    if (this.ended) {
      return { value: undefined, done: true };
    }
    return new Promise((resolve, reject) => {
      this.resolvers.push({ resolve, reject });
    });
  }
}

function createMockTransport() {
  const incomingBidirectionalStreams = new MockReadableStream<WebTransportBidirectionalStream>();
  const datagramReadable = new MockReadableStream<Uint8Array>();
  let resolveClosed: (() => void) | null = null;
  let rejectClosed: ((error: unknown) => void) | null = null;
  const closed = new Promise<void>((resolve, reject) => {
    resolveClosed = resolve;
    rejectClosed = reject;
  });
  return {
    ready: Promise.resolve(),
    closed,
    close: vi.fn(),
    incomingBidirectionalStreams,
    datagrams: {
      readable: datagramReadable,
    },
    resolveClosed: () => resolveClosed?.(),
    rejectClosed: (error: unknown) => rejectClosed?.(error),
  };
}

function createRuntime(overrides: Partial<ConstructorParameters<typeof SessionTransportRuntime>[0]> = {}) {
  const onConnect = vi.fn();
  const onDisconnect = vi.fn();
  const onError = vi.fn();
  const onStream = vi.fn();
  const onDatagram = vi.fn();
  const sendPing = vi.fn();

  const runtime = new SessionTransportRuntime({
    onConnect,
    onDisconnect,
    onError,
    onStream,
    onDatagram,
    sendPing,
    ...overrides,
  });

  return {
    runtime,
    onConnect,
    onDisconnect,
    onError,
    onStream,
    onDatagram,
    sendPing,
  };
}

describe('SessionTransportRuntime', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('constructs WebTransport with a nonce URL', async () => {
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const { runtime, onConnect } = createRuntime({
      createTransport,
      pingIntervalMs: 1000,
    });

    await runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
    });

    expect(createTransport).toHaveBeenCalledOnce();
    const [url, options] = createTransport.mock.calls[0];
    expect(url).toMatch(/^https:\/\/localhost:4433\?token=test-token&_=\d+\.\w+$/);
    expect(options).toEqual({});
    expect(onConnect).toHaveBeenCalledOnce();
  });

  it('passes server certificate hashes when fetch returns a valid hash', async () => {
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const certHashBytes = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    const fetchFn = vi.fn().mockResolvedValue({
      ok: true,
      text: async () => btoa(String.fromCharCode(...certHashBytes)),
    });
    const { runtime } = createRuntime({
      createTransport,
      fetchFn,
      pingIntervalMs: 1000,
    });

    await runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
      certHashUrl: '/cert-hash',
    });

    expect(fetchFn).toHaveBeenCalledWith('/cert-hash');
    const [, options] = createTransport.mock.calls[0];
    expect(options).toEqual({
      serverCertificateHashes: [{
        algorithm: 'sha-256',
        value: certHashBytes.buffer,
      }],
    });
  });

  it('binds browser globals when loading certificate hashes', async () => {
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const certHashBytes = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    const originalFetch = globalThis.fetch;
    const originalAtob = globalThis.atob;
    (globalThis as typeof globalThis & { fetch: typeof fetch }).fetch = vi.fn(function fetchWithReceiver(
      this: unknown,
      url: string | URL | Request,
    ) {
      expect(this).toBe(globalThis);
      expect(url).toBe('/cert-hash');
      return Promise.resolve({
        ok: true,
        text: async () => btoa(String.fromCharCode(...certHashBytes)),
      } as Response);
    }) as typeof fetch;
    (globalThis as typeof globalThis & { atob: typeof atob }).atob = vi.fn(function atobWithReceiver(
      this: unknown,
      data: string,
    ) {
      expect(this).toBe(globalThis);
      return originalAtob.call(globalThis, data);
    }) as typeof atob;

    try {
      const { runtime } = createRuntime({
        createTransport,
        pingIntervalMs: 1000,
      });
      await runtime.connect({
        gatewayUrl: 'https://localhost:4433',
        token: 'test-token',
        certHashUrl: '/cert-hash',
      });

      const [, options] = createTransport.mock.calls[0];
      expect(options).toEqual({
        serverCertificateHashes: [{
          algorithm: 'sha-256',
          value: certHashBytes.buffer,
        }],
      });
    } finally {
      (globalThis as typeof globalThis & { fetch: typeof fetch }).fetch = originalFetch;
      (globalThis as typeof globalThis & { atob: typeof atob }).atob = originalAtob;
    }
  });

  it('forwards incoming streams and datagrams to callbacks', async () => {
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const { runtime, onStream, onDatagram } = createRuntime({
      createTransport,
      pingIntervalMs: 1000,
    });

    await runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
    });

    const stream = {} as WebTransportBidirectionalStream;
    transport.incomingBidirectionalStreams.pushValue(stream);
    transport.datagrams.readable.pushValue(new Uint8Array([0x01, 0x02]));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onStream).toHaveBeenCalledWith(stream);
    expect(onDatagram).toHaveBeenCalledWith(new Uint8Array([0x01, 0x02]));
  });

  it('passes through Uint8Array datagrams without cloning them', async () => {
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const { runtime, onDatagram } = createRuntime({
      createTransport,
      pingIntervalMs: 1000,
    });

    await runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
    });

    const datagram = new Uint8Array([0x03, 0x04]);
    transport.datagrams.readable.pushValue(datagram);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onDatagram).toHaveBeenCalledWith(datagram);
    expect(onDatagram.mock.calls[0]?.[0]).toBe(datagram);
  });

  it('sends periodic pings and stops after disconnect', async () => {
    vi.useFakeTimers();
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const { runtime, sendPing } = createRuntime({
      createTransport,
      setIntervalFn: window.setInterval.bind(window),
      clearIntervalFn: window.clearInterval.bind(window),
      pingIntervalMs: 25,
    });

    await runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
    });

    await vi.advanceTimersByTimeAsync(60);
    expect(sendPing).toHaveBeenCalledTimes(2);

    runtime.disconnect();
    await vi.advanceTimersByTimeAsync(60);
    expect(sendPing).toHaveBeenCalledTimes(2);
    expect(transport.close).toHaveBeenCalledOnce();
  });

  it('reports transport close and error outcomes', async () => {
    const transport = createMockTransport();
    const createTransport = vi.fn((url: string, options: WebTransportOptions) => {
      void url;
      void options;
      return transport as unknown as WebTransport;
    });
    const { runtime, onDisconnect, onError } = createRuntime({
      createTransport,
      pingIntervalMs: 1000,
    });

    await runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
    });

    transport.resolveClosed();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(onDisconnect).toHaveBeenCalledWith('transport closed');

    const erroredTransport = createMockTransport();
    const runtimeWithError = createRuntime({
      createTransport: vi.fn((url: string, options: WebTransportOptions) => {
        void url;
        void options;
        return erroredTransport as unknown as WebTransport;
      }),
      pingIntervalMs: 1000,
    });

    await runtimeWithError.runtime.connect({
      gatewayUrl: 'https://localhost:4433',
      token: 'test-token',
    });

    const error = new Error('boom');
    erroredTransport.rejectClosed(error);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(runtimeWithError.onError).toHaveBeenCalledWith(error);
    expect(runtimeWithError.onDisconnect).toHaveBeenCalledWith('transport error');
    expect(onError).not.toHaveBeenCalled();
  });

  it('rejects connect when WebTransport is unavailable', async () => {
    const { runtime, onError } = createRuntime({
      createTransport: undefined,
      pingIntervalMs: 1000,
    });
    const originalWebTransport = (globalThis as { WebTransport?: unknown }).WebTransport;
    delete (globalThis as { WebTransport?: unknown }).WebTransport;

    try {
      await expect(runtime.connect({
        gatewayUrl: 'https://localhost:4433',
        token: 'test-token',
      })).rejects.toEqual(new UnsupportedFeatureError(
        'bpane.transport.webtransport_unavailable',
        'WebTransport is unavailable in this browser',
      ));
      expect(onError).toHaveBeenCalledTimes(1);
    } finally {
      if (originalWebTransport !== undefined) {
        (globalThis as { WebTransport?: unknown }).WebTransport = originalWebTransport;
      }
    }
  });
});
