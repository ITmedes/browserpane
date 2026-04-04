/**
 * Tests for BpaneSession class with mocked browser APIs.
 *
 * Mocks: WebTransport, VideoDecoder, AudioContext, AudioWorkletNode,
 *        ResizeObserver, navigator.clipboard, navigator.mediaDevices
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  encodeFrame,
  CH_CONTROL,
  CH_CURSOR,
  CH_CLIPBOARD,
  CH_VIDEO,
  CH_VIDEO_IN,
  CH_AUDIO_OUT,
  CH_INPUT,
  INPUT_MOUSE_MOVE,
  INPUT_KEY_EVENT_EX,
} from '../protocol.js';
import { AudioController } from '../audio-controller.js';
import { CameraController } from '../camera-controller.js';
import { installCanvasGetContextMock } from './canvas-test-helpers.js';

// ── Mock WebTransport ──────────────────────────────────────────────

class MockWritableStream {
  private writer = new MockWriter(this);
  chunks: Uint8Array[] = [];

  getWriter() { return this.writer; }
}

class MockWriter {
  private stream: MockWritableStream;
  desiredSize: number | null = 1;
  constructor(stream: MockWritableStream) { this.stream = stream; }
  async write(data: Uint8Array) { this.stream.chunks.push(new Uint8Array(data)); }
  releaseLock() {}
  close() {}
}

class MockReadableStream {
  private controller: any = null;
  private _reader: MockReader | null = null;

  constructor() {
    // We'll push values via pushValue
  }

  getReader(): MockReader {
    if (!this._reader) this._reader = new MockReader();
    return this._reader;
  }

  pushValue(value: any) {
    this._reader?.pushValue(value);
  }

  end() {
    this._reader?.end();
  }
}

class MockReader {
  private queue: Array<{ value: any; done: boolean }> = [];
  private resolvers: Array<(result: { value: any; done: boolean }) => void> = [];
  private ended = false;

  pushValue(value: any) {
    const item = { value, done: false };
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!(item);
    } else {
      this.queue.push(item);
    }
  }

  end() {
    this.ended = true;
    const item = { value: undefined, done: true };
    if (this.resolvers.length > 0) {
      this.resolvers.shift()!(item);
    } else {
      this.queue.push(item);
    }
  }

  async read(): Promise<{ value: any; done: boolean }> {
    if (this.queue.length > 0) {
      return this.queue.shift()!;
    }
    if (this.ended) {
      return { value: undefined, done: true };
    }
    return new Promise((resolve) => {
      this.resolvers.push(resolve);
    });
  }

  cancel() {}
}

class MockBidiStream {
  writable = new MockWritableStream();
  readable = new MockReadableStream();
}

function createMockTransport() {
  const bidiStream = new MockBidiStream();
  const incomingBidi = new MockReadableStream();
  const datagramReadable = new MockReadableStream();
  const closedPromise = new Promise<void>(() => {}); // never resolves by default

  const transport = {
    ready: Promise.resolve(),
    closed: closedPromise,
    close: vi.fn(),
    incomingBidirectionalStreams: {
      getReader: () => incomingBidi.getReader(),
    },
    datagrams: {
      readable: {
        getReader: () => datagramReadable.getReader(),
      },
    },
    _bidiStream: bidiStream,
    _incomingBidi: incomingBidi,
    _datagramReadable: datagramReadable,
  };

  return transport;
}

// ── Mock VideoDecoder ──────────────────────────────────────────────

class MockVideoDecoder {
  static instances: MockVideoDecoder[] = [];
  outputCallback: ((frame: any) => void) | null = null;
  errorCallback: ((e: any) => void) | null = null;
  configured = false;
  decodedChunks: any[] = [];
  decodeQueueSize = 0;
  state = 'unconfigured';

  constructor(init: { output: (frame: any) => void; error: (e: any) => void }) {
    this.outputCallback = init.output;
    this.errorCallback = init.error;
    MockVideoDecoder.instances.push(this);
  }

  configure(config: any) {
    this.configured = true;
    this.state = 'configured';
  }

  decode(chunk: any) {
    this.decodedChunks.push(chunk);
    // Simulate outputting a frame
    if (this.outputCallback) {
      this.outputCallback({
        close: vi.fn(),
        timestamp: chunk.timestamp,
      });
    }
  }

  close() {
    this.state = 'closed';
  }
}

// ── Mock EncodedVideoChunk ─────────────────────────────────────────

class MockEncodedVideoChunk {
  type: string;
  timestamp: number;
  data: Uint8Array;

  constructor(init: { type: string; timestamp: number; data: Uint8Array }) {
    this.type = init.type;
    this.timestamp = init.timestamp;
    this.data = new Uint8Array(init.data);
  }
}

// ── Mock AudioContext ──────────────────────────────────────────────

class MockAudioWorkletNode {
  port = { postMessage: vi.fn(), onmessage: null as any };
  connect = vi.fn();
  disconnect = vi.fn();
}

class MockAudioContext {
  state = 'running';
  sampleRate = 48000;
  destination = {};
  audioWorklet = {
    addModule: vi.fn().mockResolvedValue(undefined),
  };
  createMediaStreamSource = vi.fn(() => ({ connect: vi.fn() }));
  resume = vi.fn();
  close = vi.fn();
}

class MockEncodedVideoIngressChunk {
  type: EncodedVideoChunkType;
  timestamp: number;
  byteLength: number;
  private readonly data: Uint8Array;

  constructor(type: EncodedVideoChunkType, timestamp: number, data: Uint8Array) {
    this.type = type;
    this.timestamp = timestamp;
    this.data = data;
    this.byteLength = data.byteLength;
  }

  copyTo(destination: AllowSharedBufferSource) {
    if (destination instanceof Uint8Array) {
      destination.set(this.data);
      return;
    }
    new Uint8Array(destination as ArrayBufferLike).set(this.data);
  }
}

class MockCameraVideoEncoder {
  static isConfigSupported = vi.fn().mockResolvedValue({ supported: true });
  encodeQueueSize = 0;
  private readonly output: (chunk: EncodedVideoChunk) => void;

  constructor(init: VideoEncoderInit) {
    this.output = init.output;
  }

  configure() {}
  close() {}
  flush() { return Promise.resolve(); }
  encode(_frame: VideoFrame, options?: VideoEncoderEncodeOptions) {
    this.output(new MockEncodedVideoIngressChunk(
      options?.keyFrame ? 'key' : 'delta',
      0,
      new Uint8Array([0, 0, 0, 1, options?.keyFrame ? 0x65 : 0x41]),
    ) as unknown as EncodedVideoChunk);
  }
}

class MockCameraVideoFrame {
  close() {}
}

// ── Setup/teardown ─────────────────────────────────────────────────

let mockTransport: ReturnType<typeof createMockTransport>;
let microphoneSupportSpy: ReturnType<typeof vi.spyOn>;
let cameraSupportSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  mockTransport = createMockTransport();
  microphoneSupportSpy = vi.spyOn(AudioController, 'isMicrophoneSupported').mockResolvedValue(true);
  cameraSupportSpy = vi.spyOn(CameraController, 'isSupported').mockResolvedValue(true);
  installCanvasGetContextMock();

  // Install mocks on global
  (globalThis as any).WebTransport = vi.fn(() => mockTransport);
  (globalThis as any).VideoDecoder = MockVideoDecoder;
  (globalThis as any).EncodedVideoChunk = MockEncodedVideoChunk;
  (globalThis as any).AudioContext = MockAudioContext;
  (globalThis as any).AudioWorkletNode = MockAudioWorkletNode;
  (globalThis as any).VideoEncoder = MockCameraVideoEncoder;
  (globalThis as any).VideoFrame = MockCameraVideoFrame;
  (globalThis as any).ImageData = class MockImageData {
    data: Uint8ClampedArray;
    width: number;
    height: number;

    constructor(data: Uint8ClampedArray, width: number, height: number) {
      this.data = data;
      this.width = width;
      this.height = height;
    }
  };

  // ResizeObserver mock
  (globalThis as any).ResizeObserver = vi.fn(() => ({
    observe: vi.fn(),
    disconnect: vi.fn(),
    unobserve: vi.fn(),
  }));

  // Clipboard mock
  Object.defineProperty(navigator, 'clipboard', {
    value: {
      writeText: vi.fn().mockResolvedValue(undefined),
      readText: vi.fn().mockResolvedValue(''),
    },
    configurable: true,
  });
  (globalThis.navigator as any).mediaDevices = {
    getUserMedia: vi.fn().mockResolvedValue({
      getTracks: () => [{ stop: vi.fn() }],
    } as unknown as MediaStream),
  };

  // URL mock
  (globalThis as any).URL.createObjectURL = vi.fn(() => 'blob:mock');
  (globalThis as any).URL.revokeObjectURL = vi.fn();

  // requestAnimationFrame mock
  (globalThis as any).requestAnimationFrame = vi.fn((cb: () => void) => {
    // Don't call cb to avoid infinite loop in tests
    return 1;
  });

  MockVideoDecoder.instances = [];
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ── Helper to create a connected session ───────────────────────────

async function createSession(overrides: Record<string, any> = {}) {
  const container = document.createElement('div');
  Object.defineProperty(container, 'clientWidth', { value: 800 });
  Object.defineProperty(container, 'clientHeight', { value: 600 });
  container.getBoundingClientRect = () => ({
    width: 800, height: 600, top: 0, left: 0, right: 800, bottom: 600,
    x: 0, y: 0, toJSON: () => {},
  });

  const { BpaneSession } = await import('../bpane.js');

  const session = await BpaneSession.connect({
    container,
    gatewayUrl: 'https://localhost:4433',
    token: 'test-token',
    hiDpi: false,
    clipboard: true,
    ...overrides,
  });

  return { session, container };
}

// ── Tests ──────────────────────────────────────────────────────────

describe('BpaneSession', () => {
  describe('connection lifecycle', () => {
    it('creates canvas and cursor overlay on connect', async () => {
      const { container } = await createSession();
      const canvases = container.querySelectorAll('canvas');
      expect(canvases.length).toBe(2); // main canvas + cursor overlay
    });

    it('sets canvas to 100% width/height', async () => {
      const { container } = await createSession();
      const canvas = container.querySelector('canvas')!;
      expect(canvas.style.width).toBe('100%');
      expect(canvas.style.height).toBe('100%');
    });

    it('hides native cursor on canvas', async () => {
      const { container } = await createSession();
      const canvas = container.querySelector('canvas')!;
      expect(canvas.style.cursor).toBe('none');
    });

    it('calls onConnect callback', async () => {
      const onConnect = vi.fn();
      await createSession({ onConnect });
      expect(onConnect).toHaveBeenCalledOnce();
    });

    it('reports render diagnostics for embedding clients', async () => {
      const { session } = await createSession();
      const diagnostics = session.getRenderDiagnostics();
      expect(diagnostics.backend).toBe('canvas2d');
      expect(diagnostics.reason).not.toBe('hardware-accelerated');
    });

    it('constructs WebTransport with correct URL', async () => {
      await createSession();
      expect(globalThis.WebTransport).toHaveBeenCalledOnce();
      const [url, opts] = (globalThis.WebTransport as ReturnType<typeof vi.fn>).mock.calls[0];
      // URL includes token and a unique nonce to prevent QUIC connection pooling
      expect(url).toMatch(/^https:\/\/localhost:4433\?token=test-token&_=\d+\.\w+$/);
      expect(opts).toEqual({});
    });

    it('removes canvas and cursor overlay on disconnect', async () => {
      const { session, container } = await createSession();
      session.disconnect();
      const canvases = container.querySelectorAll('canvas');
      expect(canvases.length).toBe(0);
    });

    it('calls onDisconnect on disconnect', async () => {
      const onDisconnect = vi.fn();
      const { session } = await createSession({ onDisconnect });
      session.disconnect();
      expect(onDisconnect).toHaveBeenCalledWith('user disconnected');
    });

    it('closes transport on disconnect', async () => {
      const { session } = await createSession();
      session.disconnect();
      expect(mockTransport.close).toHaveBeenCalled();
    });
  });

  describe('stream handling', () => {
    it('sets up send writer on first bidi stream', async () => {
      await createSession();
      // Push a bidi stream
      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);

      // Wait a tick for async handling
      await new Promise(r => setTimeout(r, 10));

      // Session should have written initial resize request to the stream
      const chunks = mockTransport._bidiStream.writable.chunks;
      expect(chunks.length).toBeGreaterThan(0);

      // First chunk should be a CONTROL frame (ResolutionRequest)
      const first = chunks[0];
      expect(first[0]).toBe(CH_CONTROL);
    });

    it('processes incoming control frames', async () => {
      const onResolutionChange = vi.fn();
      await createSession({ onResolutionChange });

      // Push a bidi stream
      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      // Send a ResolutionAck via the readable side
      const ackPayload = new Uint8Array(5);
      ackPayload[0] = 0x02; // ResolutionAck
      ackPayload[1] = 0x00; ackPayload[2] = 0x05; // 1280
      ackPayload[3] = 0x00; ackPayload[4] = 0x03; // 768
      const frame = encodeFrame(CH_CONTROL, ackPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      await new Promise(r => setTimeout(r, 10));
      expect(onResolutionChange).toHaveBeenCalledWith(1280, 768);
    });

    it('handles SessionReady with flags', async () => {
      await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      // Send SessionReady with KEYBOARD_LAYOUT flag
      const readyPayload = new Uint8Array(3);
      readyPayload[0] = 0x03; // SessionReady
      readyPayload[1] = 1;    // version
      readyPayload[2] = 0x20; // flags: keyboard layout
      const frame = encodeFrame(CH_CONTROL, readyPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      await new Promise(r => setTimeout(r, 10));
      // Should send layout hint (keyboard layout frame on CONTROL channel)
      const chunks = mockTransport._bidiStream.writable.chunks;
      // Look for the CTRL_KEYBOARD_LAYOUT_INFO message (tag 0x06)
      const layoutFrame = chunks.find(c => c[0] === CH_CONTROL && c.length > 5 && c[5] === 0x06);
      expect(layoutFrame).toBeDefined();
    });

    it('reports session capabilities from SessionReady', async () => {
      const onCapabilitiesChange = vi.fn();
      await createSession({ onCapabilitiesChange });

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const readyPayload = new Uint8Array(3);
      readyPayload[0] = 0x03; // SessionReady
      readyPayload[1] = 1;    // version
      readyPayload[2] = 0x1d; // flags: audio + file transfer + microphone + camera
      const frame = encodeFrame(CH_CONTROL, readyPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      await new Promise(r => setTimeout(r, 10));
      expect(onCapabilitiesChange).toHaveBeenCalledWith({
        audio: true,
        microphone: true,
        camera: true,
        fileTransfer: true,
        keyboardLayout: false,
      });
    });

    it('suppresses camera capability when the browser lacks H.264 encoder support', async () => {
      const onCapabilitiesChange = vi.fn();
      cameraSupportSpy.mockResolvedValue(false);
      await createSession({ onCapabilitiesChange });

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const readyPayload = new Uint8Array(3);
      readyPayload[0] = 0x03; // SessionReady
      readyPayload[1] = 1;
      readyPayload[2] = 0x11; // audio + camera
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, readyPayload));

      await new Promise(r => setTimeout(r, 10));
      expect(onCapabilitiesChange).toHaveBeenCalledWith({
        audio: true,
        microphone: false,
        camera: false,
        fileTransfer: false,
        keyboardLayout: false,
      });
    });

    it('suppresses microphone capability when the browser lacks Opus encoder support', async () => {
      const onCapabilitiesChange = vi.fn();
      microphoneSupportSpy.mockResolvedValue(false);
      await createSession({ onCapabilitiesChange });

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const readyPayload = new Uint8Array(3);
      readyPayload[0] = 0x03; // SessionReady
      readyPayload[1] = 1;
      readyPayload[2] = 0x09; // audio + microphone
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, readyPayload));

      await new Promise(r => setTimeout(r, 10));
      expect(onCapabilitiesChange).toHaveBeenCalledWith({
        audio: true,
        microphone: false,
        camera: false,
        fileTransfer: false,
        keyboardLayout: false,
      });
    });

    it('drops viewer-only capabilities after ResolutionLocked', async () => {
      const onCapabilitiesChange = vi.fn();
      await createSession({ onCapabilitiesChange });

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const readyPayload = new Uint8Array(3);
      readyPayload[0] = 0x03; // SessionReady
      readyPayload[1] = 1;
      readyPayload[2] = 0x1d; // audio + file transfer + microphone + camera
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, readyPayload));
      await new Promise(r => setTimeout(r, 10));

      const lockedPayload = new Uint8Array(5);
      lockedPayload[0] = 0x08; // ResolutionLocked
      lockedPayload[1] = 0x00; lockedPayload[2] = 0x05; // 1280
      lockedPayload[3] = 0xD0; lockedPayload[4] = 0x02; // 720
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, lockedPayload));

      await new Promise(r => setTimeout(r, 10));
      expect(onCapabilitiesChange).toHaveBeenLastCalledWith({
        audio: true,
        microphone: false,
        camera: false,
        fileTransfer: false,
        keyboardLayout: false,
      });
    });

    it('ignores clipboard updates for viewer sessions', async () => {
      await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const lockedPayload = new Uint8Array(5);
      lockedPayload[0] = 0x08; // ResolutionLocked
      lockedPayload[1] = 0x00; lockedPayload[2] = 0x05; // 1280
      lockedPayload[3] = 0xD0; lockedPayload[4] = 0x02; // 720
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, lockedPayload));
      await new Promise(r => setTimeout(r, 10));

      const writeText = vi.mocked(navigator.clipboard.writeText);
      writeText.mockClear();

      const text = new TextEncoder().encode('viewer clipboard');
      const clipPayload = new Uint8Array(5 + text.length);
      clipPayload[0] = 0x01;
      clipPayload[1] = text.length & 0xFF;
      clipPayload[2] = (text.length >> 8) & 0xFF;
      clipPayload[3] = (text.length >> 16) & 0xFF;
      clipPayload[4] = (text.length >> 24) & 0xFF;
      clipPayload.set(text, 5);
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CLIPBOARD, clipPayload));

      await new Promise(r => setTimeout(r, 10));
      expect(writeText).not.toHaveBeenCalled();
    });

    it('responds to Ping with Pong', async () => {
      await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      // Send a Ping
      const pingPayload = new Uint8Array(13);
      pingPayload[0] = 0x04; // Ping
      const pingView = new DataView(pingPayload.buffer);
      pingView.setUint32(1, 99, true); // seq=99
      pingView.setUint32(5, 12345, true); // timestamp low
      pingView.setUint32(9, 0, true); // timestamp high
      const frame = encodeFrame(CH_CONTROL, pingPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      await new Promise(r => setTimeout(r, 10));

      // Look for Pong response
      const chunks = mockTransport._bidiStream.writable.chunks;
      const pongFrame = chunks.find(c => {
        if (c[0] !== CH_CONTROL) return false;
        // Parse inner payload
        const len = c[1] | (c[2] << 8) | (c[3] << 16) | (c[4] << 24);
        if (len < 13) return false;
        return c[5] === 0x05; // Pong tag
      });
      expect(pongFrame).toBeDefined();
      // Verify seq was echoed
      if (pongFrame) {
        const pongView = new DataView(pongFrame.buffer, pongFrame.byteOffset + 6, 4);
        expect(pongView.getUint32(0, true)).toBe(99);
      }
    });

    it('keeps keyboard input frames under backpressure', async () => {
      const { session } = await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const writer = mockTransport._bidiStream.writable.getWriter();
      writer.desiredSize = 0;

      const payload = new Uint8Array(11);
      payload[0] = INPUT_KEY_EVENT_EX;
      payload[1] = 18; // KeyE
      payload[5] = 0; // key up
      payload[7] = 0xE9; // 'é'

      const chunks = mockTransport._bidiStream.writable.chunks;
      const initialCount = chunks.length;
      (session as any).sendFrame(CH_INPUT, payload);

      expect(chunks).toHaveLength(initialCount + 1);
      expect(chunks.at(-1)).toEqual(encodeFrame(CH_INPUT, payload));
    });

    it('drops mouse-move input frames under backpressure', async () => {
      const { session } = await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const writer = mockTransport._bidiStream.writable.getWriter();
      writer.desiredSize = 0;

      const payload = new Uint8Array(5);
      payload[0] = INPUT_MOUSE_MOVE;
      payload[1] = 0x10;
      payload[3] = 0x20;

      const chunks = mockTransport._bidiStream.writable.chunks;
      const initialCount = chunks.length;
      (session as any).sendFrame(CH_INPUT, payload);

      expect(chunks).toHaveLength(initialCount);
    });

    it('keeps only the latest pending camera frame under backpressure', async () => {
      vi.useFakeTimers();
      const { session } = await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await vi.advanceTimersByTimeAsync(10);

      const writer = mockTransport._bidiStream.writable.getWriter();
      writer.desiredSize = 0;

      expect((session as any).sendCameraFrame(new Uint8Array([0x01]))).toBe('queued');
      expect((session as any).sendCameraFrame(new Uint8Array([0x02]))).toBe('replaced');
      expect(mockTransport._bidiStream.writable.chunks.at(-1)).not.toEqual(encodeFrame(CH_VIDEO_IN, new Uint8Array([0x01])));

      writer.desiredSize = 1;
      await vi.advanceTimersByTimeAsync(60);

      expect(mockTransport._bidiStream.writable.chunks.at(-1)).toEqual(encodeFrame(CH_VIDEO_IN, new Uint8Array([0x02])));
      vi.useRealTimers();
    });
  });

  describe('clipboard', () => {
    it('writes received clipboard text to navigator.clipboard', async () => {
      await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const text = 'hello clipboard';
      const encoded = new TextEncoder().encode(text);
      const clipPayload = new Uint8Array(5 + encoded.length);
      clipPayload[0] = 0x01; // CLIP_TEXT
      clipPayload[1] = encoded.length & 0xFF;
      clipPayload[2] = (encoded.length >> 8) & 0xFF;
      clipPayload[3] = 0;
      clipPayload[4] = 0;
      clipPayload.set(encoded, 5);
      const frame = encodeFrame(CH_CLIPBOARD, clipPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      await new Promise(r => setTimeout(r, 10));
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('hello clipboard');
    });

    it('does not write clipboard when clipboard option is false', async () => {
      await createSession({ clipboard: false });

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const text = 'test';
      const encoded = new TextEncoder().encode(text);
      const clipPayload = new Uint8Array(5 + encoded.length);
      clipPayload[0] = 0x01;
      clipPayload[1] = encoded.length;
      clipPayload.set(encoded, 5);
      const frame = encodeFrame(CH_CLIPBOARD, clipPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      await new Promise(r => setTimeout(r, 10));
      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    });
  });

  describe('cursor', () => {
    it('processes CursorMove messages', async () => {
      await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const cursorPayload = new Uint8Array(5);
      const view = new DataView(cursorPayload.buffer);
      view.setUint8(0, 0x01); // CursorMove
      view.setUint16(1, 100, true);
      view.setUint16(3, 200, true);
      const frame = encodeFrame(CH_CURSOR, cursorPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      // Should not throw
      await new Promise(r => setTimeout(r, 10));
    });

    it('processes CursorShape messages', async () => {
      await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const width = 16;
      const height = 16;
      const dataLen = width * height * 4;
      const cursorPayload = new Uint8Array(11 + dataLen);
      const view = new DataView(cursorPayload.buffer);
      view.setUint8(0, 0x02); // CursorShape
      view.setUint16(1, width, true);
      view.setUint16(3, height, true);
      view.setUint8(5, 8);  // hotspot_x
      view.setUint8(6, 8);  // hotspot_y
      view.setUint32(7, dataLen, true);
      // Fill with white pixels
      for (let i = 0; i < dataLen; i += 4) {
        cursorPayload[11 + i] = 255; // R
        cursorPayload[11 + i + 1] = 255; // G
        cursorPayload[11 + i + 2] = 255; // B
        cursorPayload[11 + i + 3] = 255; // A
      }
      const frame = encodeFrame(CH_CURSOR, cursorPayload);
      mockTransport._bidiStream.readable.pushValue(frame);

      // Should not throw
      await new Promise(r => setTimeout(r, 10));
    });
  });

  describe('video decode', () => {
    function buildDatagram(opts: {
      nalId: number; fragSeq: number; fragTotal: number;
      isKeyframe: boolean; data: Uint8Array;
    }): Uint8Array {
      const buf = new Uint8Array(21 + opts.data.length);
      const view = new DataView(buf.buffer);
      view.setUint32(0, opts.nalId, true);
      view.setUint16(4, opts.fragSeq, true);
      view.setUint16(6, opts.fragTotal, true);
      view.setUint8(8, opts.isKeyframe ? 1 : 0);
      view.setUint32(9, 0, true); // pts_us low
      view.setUint32(13, 0, true); // pts_us high
      view.setUint32(17, opts.data.length, true);
      buf.set(opts.data, 21);
      return buf;
    }

    it('creates VideoDecoder on first video frame', async () => {
      await createSession();

      // SPS NAL
      const sps = buildDatagram({
        nalId: 1, fragSeq: 0, fragTotal: 1, isKeyframe: true,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f]),
      });
      // PPS NAL
      const pps = buildDatagram({
        nalId: 2, fragSeq: 0, fragTotal: 1, isKeyframe: true,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce, 0x38, 0x80]),
      });
      // IDR NAL (type 5)
      const idr = buildDatagram({
        nalId: 3, fragSeq: 0, fragTotal: 1, isKeyframe: true,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88, 0x84]),
      });

      // Send via stream (reliable path)
      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      // Send video frames via reliable stream
      const spsFrame = encodeFrame(CH_VIDEO, sps);
      const ppsFrame = encodeFrame(CH_VIDEO, pps);
      const idrFrame = encodeFrame(CH_VIDEO, idr);
      mockTransport._bidiStream.readable.pushValue(spsFrame);
      await new Promise(r => setTimeout(r, 10));
      mockTransport._bidiStream.readable.pushValue(ppsFrame);
      await new Promise(r => setTimeout(r, 10));
      mockTransport._bidiStream.readable.pushValue(idrFrame);
      await new Promise(r => setTimeout(r, 10));

      // VideoDecoder should have been created and configured
      expect(MockVideoDecoder.instances.length).toBeGreaterThan(0);
      const decoder = MockVideoDecoder.instances[0];
      expect(decoder.configured).toBe(true);
      expect(decoder.decodedChunks.length).toBe(1); // Only IDR is decoded (SPS/PPS are cached)
    });

    it('processes video datagrams', async () => {
      await createSession();

      // Send SPS, PPS, IDR via datagram
      const sps = buildDatagram({
        nalId: 1, fragSeq: 0, fragTotal: 1, isKeyframe: true,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f]),
      });
      const pps = buildDatagram({
        nalId: 2, fragSeq: 0, fragTotal: 1, isKeyframe: true,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x68, 0xce, 0x38, 0x80]),
      });
      const idr = buildDatagram({
        nalId: 3, fragSeq: 0, fragTotal: 1, isKeyframe: true,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x65, 0x88, 0x84]),
      });

      mockTransport._datagramReadable.pushValue(sps);
      mockTransport._datagramReadable.pushValue(pps);
      mockTransport._datagramReadable.pushValue(idr);

      await new Promise(r => setTimeout(r, 50));

      expect(MockVideoDecoder.instances.length).toBeGreaterThan(0);
    });

    it('filters SEI NALs (type 6)', async () => {
      await createSession();

      const sei = buildDatagram({
        nalId: 1, fragSeq: 0, fragTotal: 1, isKeyframe: false,
        data: new Uint8Array([0x00, 0x00, 0x01, 0x06, 0x05, 0x00]),
      });

      mockTransport._datagramReadable.pushValue(sei);
      await new Promise(r => setTimeout(r, 10));

      // No decoder should have been created for SEI alone
      // (SEI is accumulated, not decoded directly)
      if (MockVideoDecoder.instances.length > 0) {
        expect(MockVideoDecoder.instances[0].decodedChunks.length).toBe(0);
      }
    });
  });

  describe('cert hash', () => {
    it('passes cert hash to WebTransport options', async () => {
      // Mock fetch for cert hash
      const originalFetch = globalThis.fetch;
      const b64Hash = btoa(String.fromCharCode(...new Uint8Array(32).fill(0xAB)));
      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(b64Hash),
      }) as any;

      try {
        await createSession({ certHashUrl: '/cert-hash' });
        // WebTransport should have been called with serverCertificateHashes
        expect(globalThis.WebTransport).toHaveBeenCalledWith(
          expect.any(String),
          expect.objectContaining({
            serverCertificateHashes: expect.arrayContaining([
              expect.objectContaining({ algorithm: 'sha-256' }),
            ]),
          }),
        );
      } finally {
        globalThis.fetch = originalFetch;
      }
    });

    it('continues without cert hash when fetch fails', async () => {
      const originalFetch = globalThis.fetch;
      globalThis.fetch = vi.fn().mockRejectedValue(new Error('network error')) as any;

      try {
        await createSession({ certHashUrl: '/cert-hash' });
        // Should still connect, just without cert hash
        expect(globalThis.WebTransport).toHaveBeenCalledWith(
          expect.any(String),
          {},
        );
      } finally {
        globalThis.fetch = originalFetch;
      }
    });
  });

  describe('hiDpi scaling', () => {
    it('applies devicePixelRatio when hiDpi is true', async () => {
      Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true });
      const { container } = await createSession({ hiDpi: true });
      const canvas = container.querySelector('canvas')!;
      // 800 * 2 = 1600, 600 * 2 = 1200
      expect(canvas.width).toBe(1600);
      expect(canvas.height).toBe(1200);
    });

    it('uses 1x scale when hiDpi is false', async () => {
      Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true });
      const { container } = await createSession({ hiDpi: false });
      const canvas = container.querySelector('canvas')!;
      expect(canvas.width).toBe(800);
      expect(canvas.height).toBe(600);
    });

    it('clamps scale to max 3', async () => {
      Object.defineProperty(window, 'devicePixelRatio', { value: 5, configurable: true });
      const { container } = await createSession({ hiDpi: true });
      const canvas = container.querySelector('canvas')!;
      // 800 * 3 = 2400
      expect(canvas.width).toBe(2400);
    });
  });

  describe('ping timer', () => {
    it('sends periodic pings', async () => {
      vi.useFakeTimers();
      await createSession();

      // Set up bidi stream so sendFrame works
      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await vi.advanceTimersByTimeAsync(10);

      // Advance past ping interval (5000ms)
      await vi.advanceTimersByTimeAsync(5100);

      const chunks = mockTransport._bidiStream.writable.chunks;
      // Should have at least the initial resize + 1 ping
      const pingFrames = chunks.filter(c => {
        if (c[0] !== CH_CONTROL) return false;
        const len = c[1] | (c[2] << 8) | (c[3] << 16) | (c[4] << 24);
        return len >= 13 && c[5] === 0x04; // Ping tag
      });
      expect(pingFrames.length).toBeGreaterThanOrEqual(1);

      vi.useRealTimers();
    });

    it('stops pings after disconnect', async () => {
      vi.useFakeTimers();
      const { session } = await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await vi.advanceTimersByTimeAsync(10);

      session.disconnect();

      const chunksBefore = mockTransport._bidiStream.writable.chunks.length;
      await vi.advanceTimersByTimeAsync(10000);
      // No more chunks should be written after disconnect
      expect(mockTransport._bidiStream.writable.chunks.length).toBe(chunksBefore);

      vi.useRealTimers();
    });
  });

  describe('error handling', () => {
    it('rejects microphone start when disabled in session options', async () => {
      const { session } = await createSession({ microphone: false });
      await expect(session.startMicrophone()).rejects.toThrow('microphone input is disabled in session options');
    });

    it('rejects microphone start when browser Opus encoding is unavailable', async () => {
      microphoneSupportSpy.mockResolvedValue(false);
      const { session } = await createSession();
      await expect(session.startMicrophone()).rejects.toThrow('browser Opus microphone encoding is unavailable');
    });

    it('rejects microphone start for viewer sessions', async () => {
      const { session } = await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const lockedPayload = new Uint8Array(5);
      lockedPayload[0] = 0x08; // ResolutionLocked
      lockedPayload[1] = 0x00; lockedPayload[2] = 0x05; // 1280
      lockedPayload[3] = 0xD0; lockedPayload[4] = 0x02; // 720
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, lockedPayload));

      await new Promise(r => setTimeout(r, 10));
      await expect(session.startMicrophone()).rejects.toThrow('microphone input is disabled for viewer sessions');
    });

    it('rejects camera start when browser H.264 encoding is unavailable', async () => {
      cameraSupportSpy.mockResolvedValue(false);
      const { session } = await createSession();
      await expect(session.startCamera()).rejects.toThrow('camera video encoding is not supported in this browser');
    });

    it('rejects file upload for viewer sessions', async () => {
      const { session } = await createSession();

      mockTransport._incomingBidi.pushValue(mockTransport._bidiStream);
      await new Promise(r => setTimeout(r, 10));

      const lockedPayload = new Uint8Array(5);
      lockedPayload[0] = 0x08; // ResolutionLocked
      lockedPayload[1] = 0x00; lockedPayload[2] = 0x05; // 1280
      lockedPayload[3] = 0xD0; lockedPayload[4] = 0x02; // 720
      mockTransport._bidiStream.readable.pushValue(encodeFrame(CH_CONTROL, lockedPayload));

      await new Promise(r => setTimeout(r, 10));
      const file = new File(['viewer'], 'viewer.txt', { type: 'text/plain' });
      await expect(session.uploadFiles([file])).rejects.toThrow('file upload is disabled for viewer sessions');
    });

    it('calls onError when WebTransport fails', async () => {
      const onError = vi.fn();
      (globalThis as any).WebTransport = vi.fn(() => ({
        ready: Promise.reject(new Error('connection failed')),
        closed: new Promise(() => {}),
        close: vi.fn(),
        incomingBidirectionalStreams: { getReader: () => new MockReader() },
        datagrams: { readable: { getReader: () => new MockReader() } },
      }));

      const container = document.createElement('div');
      Object.defineProperty(container, 'clientWidth', { value: 800 });
      Object.defineProperty(container, 'clientHeight', { value: 600 });
      container.getBoundingClientRect = () => ({
        width: 800, height: 600, top: 0, left: 0, right: 800, bottom: 600,
        x: 0, y: 0, toJSON: () => {},
      });

      const { BpaneSession } = await import('../bpane.js');

      await expect(BpaneSession.connect({
        container,
        gatewayUrl: 'https://localhost:4433',
        token: 'test',
        onError,
      })).rejects.toThrow('connection failed');

      expect(onError).toHaveBeenCalled();
    });
  });
});
