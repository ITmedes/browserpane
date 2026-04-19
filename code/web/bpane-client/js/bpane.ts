/**
 * BrowserPane Client - Public TypeScript API
 *
 * Embeds a remote Linux desktop into any HTML container element.
 * This is the single source of truth for all browser-side BrowserPane logic.
 */

import {
  encodeFrame, parseFrames,
  FRAME_HEADER_SIZE,
  CH_VIDEO, CH_AUDIO_OUT, CH_AUDIO_IN, CH_VIDEO_IN, CH_INPUT, CH_CURSOR,
  CH_CLIPBOARD, CH_CONTROL, CH_FILE_UP, CH_FILE_DOWN,
  INPUT_MOUSE_MOVE,
  CTRL_KEYBOARD_LAYOUT,
} from './protocol.js';

import { NalReassembler, getNalType, type ReassembledNal, type TileInfo } from './nal.js';
import { fnvHash } from './hash.js';
import { TileCompositor, CH_TILES } from './tile-compositor.js';
import { WebGLTileRenderer, type WebGLRendererDiagnostics } from './webgl-compositor.js';
import { SessionStats } from './session-stats.js';
import { AudioController } from './audio-controller.js';
import { CameraController } from './camera-controller.js';
import { FileTransferController } from './file-transfer.js';
import { InputController } from './input-controller.js';
import { SessionCapabilityRuntime } from './session-capability-runtime.js';
import { SessionControlRuntime } from './session-control-runtime.js';
import { SessionCursorRuntime } from './session-cursor-runtime.js';
import { SessionResizeRuntime } from './session-resize-runtime.js';
import { UnsupportedFeatureError } from './shared/errors.js';
import { SessionConnectOptionsValidator } from './shared/connect-options-validator.js';

export interface BpaneOptions {
  container: HTMLElement;
  gatewayUrl: string;
  token: string;
  hiDpi?: boolean;
  audio?: boolean;
  microphone?: boolean;
  camera?: boolean;
  clipboard?: boolean;
  fileTransfer?: boolean;
  /** URL to fetch TLS cert SHA-256 hash (base64) for self-signed certs */
  certHashUrl?: string;
  onConnect?: () => void;
  onDisconnect?: (reason: string) => void;
  onError?: (error: Error) => void;
  onCapabilitiesChange?: (capabilities: SessionCapabilities) => void;
  onResolutionChange?: (width: number, height: number) => void;
}

export interface SessionCapabilities {
  audio: boolean;
  microphone: boolean;
  camera: boolean;
  fileTransfer: boolean;
  keyboardLayout: boolean;
}

// Re-export stats types for backward compatibility
export type { ChannelTransferStats, TileCommandStats } from './session-stats.js';
export type { SessionStatsSnapshot as SessionStats } from './session-stats.js';
export type { WebGLRendererDiagnostics as RenderDiagnostics } from './webgl-compositor.js';

const PING_INTERVAL_MS = 5000;
const TILE_CACHE_MISS = 0x09;
const VIDEO_OVERLAY_STALE_MS = 450;
export class BpaneSession {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null = null;
  private container: HTMLElement;
  private transport: WebTransport | null = null;
  private options: BpaneOptions;
  private connected = false;
  // Extracted: input event handling
  private input: InputController | null = null;
  private videoDecoder: VideoDecoder | null = null;
  private cursorEl: HTMLCanvasElement | null = null;
  private cursorRuntime: SessionCursorRuntime;
  private sendWritable: WritableStreamDefaultWriter<Uint8Array> | null = null;
  private pendingFrames: Uint8Array[] = [];
  private pendingCameraFrame: Uint8Array | null = null;
  private pendingCameraFlushTimer: ReturnType<typeof setTimeout> | null = null;
  // Extracted: audio output, Opus decode, and microphone
  private audio: AudioController;
  private camera: CameraController;
  private microphoneEncoderSupported = false;
  private cameraEncoderSupported = false;
  private sessionFlags = 0;
  private microphoneSupported: boolean | null = null;
  private cameraSupported: boolean | null = null;
  private capabilities: SessionCapabilities = {
    audio: false,
    microphone: false,
    camera: false,
    fileTransfer: false,
    keyboardLayout: false,
  };
  private capabilityRuntime: SessionCapabilityRuntime;
  private fileTransfer: FileTransferController | null = null;
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private pingSeq = 0;
  private remoteWidth = 0;
  private remoteHeight = 0;
  // Gateway-managed access state.
  private viewerRestricted = false;
  private controlRuntime: SessionControlRuntime;
  private resizeRuntime: SessionResizeRuntime;

  // Tile compositor
  private tileCompositor = new TileCompositor();

  // WebGL2 renderer (null if WebGL2 not available, falls back to Canvas2D)
  private glRenderer: WebGLTileRenderer | null = null;
  private renderDiagnostics: WebGLRendererDiagnostics = {
    backend: 'canvas2d',
    renderer: null,
    vendor: null,
    software: false,
    reason: 'unsupported',
  };

  // NAL reassembly
  private nalReassembler = new NalReassembler();

  // Video decode state
  private decoderConfigured = false;
  private decoderTimestamp = 0;
  private spsNal: Uint8Array | null = null;
  private ppsNal: Uint8Array | null = null;
  private seiNals: Uint8Array[] = [];
  private pendingTileInfo: TileInfo | null = null;
  private currentDecodeTileInfo: TileInfo | null = null;
  private pendingVideoFrame: VideoFrame | null = null;
  private displayLoopRunning = false;
  private displayDirty = false;
  // Extracted: all transfer/tile/scroll stats live in SessionStats
  private stats = new SessionStats();

  // Persistent video buffer: holds the latest decoded video frame so it can
  // be redrawn on top of tiles every rAF, ensuring correct z-ordering.
  private videoBuffer: HTMLCanvasElement | null = null;
  private videoBufferCtx: CanvasRenderingContext2D | null = null;
  private videoBufferTileInfo: TileInfo | null = null;
  private lastVideoFrameAtMs = 0;

  private constructor(options: BpaneOptions) {
    this.options = options;
    this.container = options.container;
    this.audio = new AudioController(
      options.audio ?? true,
      (channelId, payload) => this.sendFrame(channelId, payload),
    );
    this.camera = new CameraController(
      (payload) => this.sendCameraFrame(payload),
    );
    this.fileTransfer = new FileTransferController({
      container: this.container,
      enabled: options.fileTransfer !== false,
      sendFrame: (channelId, payload) => this.sendFrame(channelId, payload),
    });
    this.capabilityRuntime = new SessionCapabilityRuntime({
      fileTransferOptionEnabled: options.fileTransfer !== false,
      stopMicrophone: () => this.audio.stopMicrophone(),
      stopCamera: () => this.camera.stopCamera(),
      setFileTransferEnabled: (enabled) => {
        this.fileTransfer?.setEnabled(enabled);
      },
      onCapabilitiesChange: (capabilities) => {
        this.options.onCapabilitiesChange?.(capabilities);
      },
    });
    this.controlRuntime = new SessionControlRuntime({
      setRemoteSize: (width, height) => {
        this.remoteWidth = width;
        this.remoteHeight = height;
      },
      onResolutionChange: (width, height) => {
        this.options.onResolutionChange?.(width, height);
      },
      setSessionFlags: (flags) => {
        this.sessionFlags = flags;
      },
      setMicrophoneSupported: (supported) => {
        this.microphoneSupported = supported;
      },
      setCameraSupported: (supported) => {
        this.cameraSupported = supported;
      },
      configureInputExtendedKeyEvents: (enabled) => {
        if (this.input) {
          this.input.serverSupportsKeyEventEx = enabled;
        }
      },
      sendLayoutHint: () => {
        this.input?.sendLayoutHint();
      },
      updateCapabilities: () => {
        this.updateCapabilities();
      },
      applyClientAccessState: (flags, width, height) => {
        this.applyClientAccessState(flags, width, height);
      },
      sendControlFrame: (payload) => {
        this.sendFrame(CH_CONTROL, payload);
      },
    });

    // Create canvas element
    this.canvas = document.createElement('canvas');
    this.canvas.style.width = '100%';
    this.canvas.style.height = '100%';
    this.canvas.style.display = 'block';
    this.canvas.style.cursor = 'none';
    this.canvas.tabIndex = 0;
    this.container.appendChild(this.canvas);

    // Cursor overlay
    this.cursorEl = document.createElement('canvas');
    this.cursorEl.style.position = 'absolute';
    this.cursorEl.style.pointerEvents = 'none';
    this.cursorEl.style.top = '0';
    this.cursorEl.style.left = '0';
    this.cursorEl.style.width = '100%';
    this.cursorEl.style.height = '100%';
    this.cursorEl.width = Math.max(64, Math.floor(this.container.clientWidth));
    this.cursorEl.height = Math.max(64, Math.floor(this.container.clientHeight));
    this.cursorEl.style.zIndex = '2';
    const cursorCtx = this.cursorEl.getContext('2d');
    this.container.style.position = 'relative';
    this.container.appendChild(this.cursorEl);
    this.cursorRuntime = new SessionCursorRuntime({
      canvas: this.canvas,
      cursorEl: this.cursorEl,
      cursorCtx,
    });

    // Try WebGL2 first for GPU-accelerated tile compositing, fall back to Canvas2D.
    const webgl = WebGLTileRenderer.tryCreate(this.canvas);
    this.glRenderer = webgl.renderer;
    this.renderDiagnostics = webgl.diagnostics;
    if (this.glRenderer) {
      this.tileCompositor.setWebGLRenderer(this.glRenderer);
      // ctx stays null — Canvas2D is not used when WebGL is active
    } else {
      this.ctx = this.canvas.getContext('2d', {
        alpha: false,
        desynchronized: true,
      });
      if (this.ctx) this.tileCompositor.setContext(this.ctx);
    }
    this.tileCompositor.setCacheMissHandler(({ frameSeq, col, row, hash }) => {
      this.sendTileCacheMiss(frameSeq, col, row, hash);
    });

    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        this.resizeRuntime.handleResize(Math.floor(width), Math.floor(height));
      }
    });
    this.resizeRuntime = new SessionResizeRuntime({
      container: this.container,
      canvas: this.canvas,
      cursorEl: this.cursorEl,
      hiDpi: options.hiDpi ?? false,
      resizeObserver,
      resizeRenderer: (width, height) => {
        if (this.glRenderer) {
          this.glRenderer.resize(width, height);
        }
      },
      markDisplayDirty: () => {
        this.displayDirty = true;
      },
      sendResizeRequest: (width, height) => {
        this.sendResizeRequest(width, height);
      },
      setRemoteSize: (width, height) => {
        this.remoteWidth = width;
        this.remoteHeight = height;
      },
      onResolutionChange: (width, height) => {
        this.options.onResolutionChange?.(width, height);
      },
    });
    this.resizeRuntime.initializeCanvasSize();
    resizeObserver.observe(this.container);

    // Start display loop
    this.startDisplayLoop();
  }

  /**
   * Connect to a BrowserPane gateway and start a remote desktop session.
   */
  static async connect(options: BpaneOptions): Promise<BpaneSession> {
    SessionConnectOptionsValidator.validate(options);
    const session = new BpaneSession(options);
    session.microphoneEncoderSupported = await AudioController.isMicrophoneSupported();
    session.cameraEncoderSupported = await CameraController.isSupported();
    await session.setupTransport();
    session.input = new InputController({
      canvas: session.canvas,
      sendFrame: (channelId, payload) => session.sendFrame(channelId, payload),
      drawCursor: (_shape, x, y) => session.cursorRuntime.drawMove(x, y),
      getRemoteDims: () => ({
        width: session.remoteWidth || session.canvas.width,
        height: session.remoteHeight || session.canvas.height,
      }),
      clipboardEnabled: options.clipboard !== false,
    });
    session.input.setup();
    return session;
  }

  /** Number of decoded video frames since connect. */
  getFrameCount(): number { return this.stats.frameCount; }

  /** Tile cache statistics for debugging. */
  getTileCacheStats(): {
    hits: number;
    misses: number;
    hitRate: number;
    size: number;
    fills: number;
    qoiDecodes: number;
    qoiRedundant: number;
    qoiRedundantBytes: number;
    zstdDecodes: number;
    zstdRedundant: number;
    zstdRedundantBytes: number;
    cacheMisses: number;
    scrollCopies: number;
  } {
    const cache = this.tileCompositor.getCache();
    const stats = this.tileCompositor.stats;
    return {
      hits: cache.hits,
      misses: cache.misses,
      hitRate: cache.hitRate,
      size: cache.size,
      fills: stats.fills,
      qoiDecodes: stats.qoiDecodes,
      qoiRedundant: stats.qoiRedundant,
      qoiRedundantBytes: stats.qoiRedundantBytes,
      zstdDecodes: stats.zstdDecodes,
      zstdRedundant: stats.zstdRedundant,
      zstdRedundantBytes: stats.zstdRedundantBytes,
      cacheMisses: stats.cacheMisses,
      scrollCopies: stats.scrollCopies,
    };
  }

  /** Per-session transfer and tile-command statistics. */
  getSessionStats(): import('./session-stats.js').SessionStatsSnapshot {
    const tileRuntime = this.getTileCacheStats();
    return this.stats.getSessionStats({
      hits: tileRuntime.hits,
      misses: tileRuntime.misses,
      hitRate: tileRuntime.hitRate,
      size: tileRuntime.size,
      qoiRedundant: tileRuntime.qoiRedundant,
      qoiRedundantBytes: tileRuntime.qoiRedundantBytes,
      zstdRedundant: tileRuntime.zstdRedundant,
      zstdRedundantBytes: tileRuntime.zstdRedundantBytes,
    }, this.camera.getStats());
  }

  /** Active render backend and WebGL selection diagnostics for embedding clients. */
  getRenderDiagnostics(): WebGLRendererDiagnostics {
    return { ...this.renderDiagnostics };
  }

  /**
   * Disconnect and clean up.
   */
  disconnect(): void {
    if (!this.connected) return;
    this.connected = false;
    this.displayLoopRunning = false;
    this.resizeRuntime.destroy();

    // Remove all DOM event listeners
    if (this.input) {
      this.input.destroy();
      this.input = null;
    }

    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }

    if (this.videoDecoder) {
      try { this.videoDecoder.close(); } catch (_) { /* ignore */ }
      this.videoDecoder = null;
    }
    this.decoderConfigured = false;

    this.camera.destroy();
    this.audio.destroy();
    if (this.fileTransfer) {
      this.fileTransfer.destroy();
      this.fileTransfer = null;
    }

    this.clearVideoOverlay();

    if (this.transport) {
      this.transport.close();
      this.transport = null;
    }

    if (this.canvas.parentNode) {
      this.canvas.parentNode.removeChild(this.canvas);
    }
    if (this.cursorEl && this.cursorEl.parentNode) {
      this.cursorEl.parentNode.removeChild(this.cursorEl);
      this.cursorEl = null;
    }
    this.cursorRuntime.reset();

    this.tileCompositor.reset();
    if (this.glRenderer) {
      this.glRenderer.destroy();
      this.glRenderer = null;
      this.tileCompositor.setWebGLRenderer(null);
    }
    if (this.pendingCameraFlushTimer) {
      clearTimeout(this.pendingCameraFlushTimer);
      this.pendingCameraFlushTimer = null;
    }
    this.sendWritable = null;
    this.pendingCameraFrame = null;
    this.remoteWidth = 0;
    this.remoteHeight = 0;
    this.microphoneEncoderSupported = false;
    this.cameraEncoderSupported = false;
    this.sessionFlags = 0;
    this.microphoneSupported = null;
    this.cameraSupported = null;
    this.capabilities = {
      audio: false,
      microphone: false,
      camera: false,
      fileTransfer: false,
      keyboardLayout: false,
    };
    this.stats.frameCount = 0;

    this.options.onDisconnect?.('user disconnected');
  }

  private clearVideoOverlay(): void {
    if (this.pendingVideoFrame) {
      this.pendingVideoFrame.close();
      this.pendingVideoFrame = null;
    }
    this.pendingTileInfo = null;
    this.currentDecodeTileInfo = null;
    this.videoBuffer = null;
    this.videoBufferCtx = null;
    this.videoBufferTileInfo = null;
    this.lastVideoFrameAtMs = 0;
  }

  private async setupTransport(): Promise<void> {
    // Unique nonce prevents Chrome from pooling this WebTransport session
    // onto an existing QUIC connection (wtransport only handles one session
    // per QUIC connection).
    const nonce = `${Date.now()}.${Math.random().toString(36).slice(2)}`;
    const url = `${this.options.gatewayUrl}?token=${encodeURIComponent(this.options.token)}&_=${nonce}`;

    // Fetch cert hash for self-signed certs
    let certHash: Uint8Array | null = null;
    if (this.options.certHashUrl) {
      certHash = await this.fetchCertHash(this.options.certHashUrl);
    }

    try {
      if (typeof WebTransport === 'undefined') {
        throw new UnsupportedFeatureError(
          'bpane.transport.webtransport_unavailable',
          'WebTransport is unavailable in this browser',
        );
      }
      const wtOptions: WebTransportOptions = {};
      if (certHash) {
        wtOptions.serverCertificateHashes = [{
          algorithm: 'sha-256',
          value: new Uint8Array(certHash).buffer,
        }];
      }
      this.transport = new WebTransport(url, wtOptions);
      await this.transport.ready;
      this.connected = true;
      this.options.onConnect?.();

      // Handle transport close
      this.transport.closed.then(() => {
        if (this.connected) {
          this.connected = false;
          this.options.onDisconnect?.('transport closed');
        }
      }).catch((e: Error) => {
        if (this.connected) {
          this.connected = false;
          this.options.onError?.(e);
          this.options.onDisconnect?.('transport error');
        }
      });

      this.readStreams();
      this.readDatagrams();
      this.startPingTimer();
    } catch (e) {
      const error = e instanceof Error ? e : new Error(String(e));
      this.options.onError?.(error);
      throw error;
    }
  }

  private async fetchCertHash(url: string): Promise<Uint8Array | null> {
    try {
      const resp = await fetch(url);
      if (!resp.ok) return null;
      const b64 = (await resp.text()).trim();
      if (b64.length < 10) return null;
      const raw = atob(b64);
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
      return bytes;
    } catch {
      return null;
    }
  }

  private async readStreams(): Promise<void> {
    if (!this.transport) return;
    const reader = this.transport.incomingBidirectionalStreams.getReader();
    try {
      while (this.connected) {
        const { value: stream, done } = await reader.read();
        if (done) break;
        if (stream) this.handleStream(stream);
      }
    } catch (e) {
      if (this.connected) {
        this.options.onError?.(e instanceof Error ? e : new Error(String(e)));
      }
    }
  }

  private async readDatagrams(): Promise<void> {
    if (!this.transport) return;
    try {
      const reader = this.transport.datagrams.readable.getReader();
      while (this.connected) {
        const { value, done } = await reader.read();
        if (done) break;
        if (value) {
          const datagram = new Uint8Array(value);
          this.stats.recordRx(CH_VIDEO, datagram.byteLength);
          this.stats.videoDatagramsRx += 1;
          this.stats.videoDatagramBytesRx += datagram.byteLength;
          this.handleVideoFrame(datagram);
        }
      }
    } catch (e) {
      if (this.connected) {
        console.error('[bpane] datagram read error:', e);
      }
    }
  }

  private startPingTimer(): void {
    this.pingInterval = setInterval(() => {
      if (!this.connected) return;
      this.pingSeq++;
      const now = BigInt(Date.now());
      const payload = new Uint8Array(13);
      const view = new DataView(payload.buffer);
      payload[0] = 0x04; // Ping tag
      view.setUint32(1, this.pingSeq, true);
      view.setUint32(5, Number(now & 0xFFFFFFFFn), true);
      view.setUint32(9, Number((now >> 32n) & 0xFFFFFFFFn), true);
      this.sendFrame(CH_CONTROL, payload);
    }, PING_INTERVAL_MS);
  }

  // ── Video decode via WebCodecs ────────────────────────────────────

  private startDisplayLoop(): void {
    if (this.displayLoopRunning) return;
    this.displayLoopRunning = true;
    const loop = () => {
      if (!this.displayDirty) {
        if (this.displayLoopRunning) requestAnimationFrame(loop);
        return;
      }
      this.displayDirty = false;

      // Step 1: If there's a new decoded video frame, capture it for re-compositing.
      if (this.pendingVideoFrame) {
        if (this.glRenderer) {
          // WebGL path: upload VideoFrame directly to GPU texture (zero-copy on Chrome).
          // No intermediate Canvas2D copy needed.
          this.glRenderer.uploadVideoFrame(this.pendingVideoFrame);
        } else {
          // Canvas2D path: copy to a persistent canvas buffer for re-compositing.
          const fw = this.pendingVideoFrame.displayWidth;
          const fh = this.pendingVideoFrame.displayHeight;
          if (!this.videoBuffer || this.videoBuffer.width !== fw || this.videoBuffer.height !== fh) {
            this.videoBuffer = document.createElement('canvas');
            this.videoBuffer.width = fw;
            this.videoBuffer.height = fh;
            this.videoBufferCtx = this.videoBuffer.getContext('2d');
          }
          if (this.videoBufferCtx) {
            this.videoBufferCtx.drawImage(this.pendingVideoFrame, 0, 0);
          }
        }
        this.pendingVideoFrame.close();
        this.videoBufferTileInfo = this.pendingTileInfo;
        this.lastVideoFrameAtMs = performance.now();
        this.pendingVideoFrame = null;
        this.pendingTileInfo = null;
      }

      // Step 2: Composite video on top of tiles. This runs every rAF so
      // even if tiles drew over the video region, video is always on top.
      const hasFreshVideo = this.lastVideoFrameAtMs > 0
        && (performance.now() - this.lastVideoFrameAtMs) <= VIDEO_OVERLAY_STALE_MS;
      if (hasFreshVideo && this.canvas) {
        if (this.glRenderer) {
          // WebGL path: redraw cached GPU video texture (no CPU round-trip)
          const tile = this.videoBufferTileInfo;
          if (tile && tile.tileW > 0 && tile.tileH > 0) {
            this.glRenderer.drawCachedVideo(tile.tileX, tile.tileY, tile.tileW, tile.tileH);
          } else if (!this.tileCompositor.getGridConfig()) {
            this.glRenderer.drawCachedVideo(0, 0, this.canvas.width, this.canvas.height);
          } else {
            const vr = this.tileCompositor.getVideoRegion();
            if (vr && vr.w > 0 && vr.h > 0) {
              this.glRenderer.drawCachedVideoCropped(
                vr.x, vr.y, vr.w, vr.h,
                vr.x, vr.y, vr.w, vr.h,
              );
            }
          }
        } else if (this.ctx && this.videoBuffer && this.videoBufferCtx) {
          // Canvas2D path (fallback)
          const tile = this.videoBufferTileInfo;
          if (tile && tile.tileW > 0 && tile.tileH > 0) {
            this.ctx.drawImage(
              this.videoBuffer,
              0, 0, this.videoBuffer.width, this.videoBuffer.height,
              tile.tileX, tile.tileY, tile.tileW, tile.tileH,
            );
          } else if (!this.tileCompositor.getGridConfig()) {
            this.ctx.drawImage(this.videoBuffer, 0, 0, this.canvas.width, this.canvas.height);
          } else {
            const vr = this.tileCompositor.getVideoRegion();
            if (vr && vr.w > 0 && vr.h > 0) {
              this.ctx.drawImage(
                this.videoBuffer,
                vr.x, vr.y, vr.w, vr.h,
                vr.x, vr.y, vr.w, vr.h,
              );
            }
          }
        }
      }

      if (this.displayLoopRunning) requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  private initDecoder(): void {
    if (this.videoDecoder) {
      try { this.videoDecoder.close(); } catch (_) { /* ignore */ }
    }
    this.videoDecoder = new VideoDecoder({
      output: (frame: VideoFrame) => {
        if ((!this.ctx && !this.glRenderer) || !this.canvas) { frame.close(); return; }
        if (this.pendingVideoFrame) this.pendingVideoFrame.close();
        this.pendingVideoFrame = frame;
        this.pendingTileInfo = this.currentDecodeTileInfo;
        this.displayDirty = true;
      },
      error: (e: DOMException) => {
        console.error('[bpane] VideoDecoder error:', e.message);
        this.decoderConfigured = false;
      },
    });
    this.videoDecoder.configure({
      codec: 'avc1.42002a', // H.264 Baseline Level 4.2
      optimizeForLatency: true,
    });
    this.decoderConfigured = true;
    this.decoderTimestamp = 0;
  }

  private decodeVideoNal(nalData: Uint8Array, isKeyframe: boolean, tileInfo: TileInfo | null): void {
    const nalType = getNalType(nalData);

    // Cache parameter sets
    if (nalType === 7) { this.spsNal = nalData; return; }
    if (nalType === 8) { this.ppsNal = nalData; return; }
    if (nalType === 6) { this.seiNals.push(nalData); return; } // SEI — accumulate

    // Only feed VCL NALs (slice types: 1=non-IDR, 5=IDR)
    if (nalType !== 1 && nalType !== 5) return;

    // Initialize decoder on first frame
    if (!this.videoDecoder) this.initDecoder();

    // If decoder errored, wait for next keyframe to reinitialize
    if (!this.decoderConfigured && nalType !== 5) return;
    if (!this.decoderConfigured) this.initDecoder();

    // Build access unit: prepend SPS+PPS+SEIs for keyframes
    let chunk: Uint8Array;
    if (nalType === 5 && this.spsNal && this.ppsNal) {
      let totalLen = this.spsNal.length + this.ppsNal.length + nalData.length;
      for (const sei of this.seiNals) totalLen += sei.length;
      chunk = new Uint8Array(totalLen);
      let off = 0;
      chunk.set(this.spsNal, off); off += this.spsNal.length;
      chunk.set(this.ppsNal, off); off += this.ppsNal.length;
      for (const sei of this.seiNals) {
        chunk.set(sei, off); off += sei.length;
      }
      chunk.set(nalData, off);
      this.seiNals = [];
    } else {
      chunk = nalData;
    }

    // Store tile info for the decoder output callback
    this.currentDecodeTileInfo = tileInfo;

    // Avoid decoder backpressure adding latency
    if (this.videoDecoder!.decodeQueueSize > 3) {
      this.stats.videoFramesDropped++;
      return;
    }

    this.stats.frameCount++;

    this.decoderTimestamp += 1;
    try {
      this.videoDecoder!.decode(new EncodedVideoChunk({
        type: nalType === 5 ? 'key' : 'delta',
        timestamp: this.decoderTimestamp,
        data: chunk,
      }));
    } catch (e) {
      console.error('[bpane] decode error:', e);
    }
  }

  // ── Stream handling ────────────────────────────────────────────────

  private async handleStream(stream: WebTransportBidirectionalStream): Promise<void> {
    if (!this.sendWritable) {
      this.sendWritable = stream.writable.getWriter();
      // Flush queued frames
      if (this.pendingFrames.length > 0) {
        const queued = this.pendingFrames.splice(0);
        queued.forEach((f) => {
          this.stats.recordTx(f[0] ?? 0, f.length);
          this.sendWritable?.write(f).catch(() => {});
        });
      }
      this.schedulePendingCameraFlush(0);
      // Send initial resolution
      const dims = this.resizeRuntime.getContainerResizeDims();
      this.sendResizeRequest(dims.width, dims.height);
    }

    const reader = stream.readable.getReader();
    let buf = new Uint8Array(128 * 1024);
    let bufLen = 0;

    try {
      while (this.connected) {
        const { value, done } = await reader.read();
        if (done) break;
        if (!value) continue;

        const chunk = new Uint8Array(value.buffer || value, value.byteOffset || 0, value.byteLength || value.length);

        // Grow buffer if needed (double or fit, whichever is larger)
        const needed = bufLen + chunk.length;
        if (needed > buf.length) {
          const newBuf = new Uint8Array(Math.max(needed, buf.length * 2));
          newBuf.set(buf.subarray(0, bufLen));
          buf = newBuf;
        }

        // Append chunk
        buf.set(chunk, bufLen);
        bufLen += chunk.length;

        // Parse frames from buf[0..bufLen] — payloads are zero-copy subarray views
        const [frames, remaining] = parseFrames(buf.subarray(0, bufLen));

        // Process frames synchronously (safe: subarray views into buf are valid until next iteration)
        for (const frame of frames) {
          this.stats.recordRx(frame.channelId, frame.payload.length + FRAME_HEADER_SIZE);
          this.handleFrame(frame.channelId, frame.payload);
        }

        // Compact: move remaining bytes to front
        if (remaining.length > 0) {
          buf.copyWithin(0, bufLen - remaining.length, bufLen);
        }
        bufLen = remaining.length;
      }
    } catch (e) {
      if (this.connected) {
        console.error('[bpane] stream read error:', e);
      }
    }
  }

  private handleFrame(channelId: number, payload: Uint8Array): void {
    switch (channelId) {
      case CH_VIDEO:
        this.handleVideoFrame(payload);
        break;
      case CH_AUDIO_OUT:
        this.audio.handleFrame(payload);
        break;
      case CH_CURSOR:
        this.handleCursorUpdate(payload);
        break;
      case CH_CLIPBOARD:
        this.handleClipboardUpdate(payload);
        break;
      case CH_CONTROL:
        this.handleControlMessage(payload);
        break;
      case CH_FILE_DOWN:
        this.fileTransfer?.handleFrame(payload);
        break;
      case CH_TILES:
        {
          this.stats.tileCommandBytes += payload.byteLength;
          const cmd = this.tileCompositor.handlePayload(payload);
          if (!cmd) {
            this.stats.tileCommandCounts.unknown += 1;
            break;
          }
          switch (cmd.type) {
            case 'grid-config':
              this.stats.tileCommandCounts.gridConfig += 1;
              this.stats.resetPendingTileBatch();
              break;
            case 'batch-end':
              this.stats.tileCommandCounts.batchEnd += 1;
              {
                const grid = this.tileCompositor.getGridConfig();
                const tileSize = grid?.tileSize ?? 64;
                const cols = grid?.cols ?? 0;
                const rows = grid?.rows ?? 0;
                this.stats.finalizePendingTileBatch(tileSize, cols, rows);
              }
              this.displayDirty = true;
              break;
            case 'fill':
              this.stats.tileCommandCounts.fill += 1;
              this.stats.pendingTileBatch.fill += 1;
              break;
            case 'qoi':
              this.stats.tileCommandCounts.qoi += 1;
              this.stats.pendingTileBatch.qoi += 1;
              this.stats.pendingTileBatch.qoiBytes += cmd.data.byteLength;
              break;
            case 'zstd':
              this.stats.tileCommandCounts.zstd += 1;
              this.stats.pendingTileBatch.qoi += 1;
              this.stats.pendingTileBatch.qoiBytes += cmd.data.byteLength;
              break;
            case 'cache-hit':
              this.stats.tileCommandCounts.cacheHit += 1;
              this.stats.pendingTileBatch.cacheHit += 1;
              break;
            case 'video-region':
              this.stats.tileCommandCounts.videoRegion += 1;
              break;
            case 'scroll-copy':
              this.stats.tileCommandCounts.scrollCopy += 1;
              this.stats.pendingTileBatch.hasScrollCopy = true;
              this.stats.pendingTileBatch.maxAbsDy = Math.max(this.stats.pendingTileBatch.maxAbsDy, Math.abs(cmd.dy));
              break;
            case 'grid-offset':
              this.stats.tileCommandCounts.gridOffset += 1;
              this.stats.pendingTileBatch.gridOffsetY = cmd.offsetY;
              break;
            case 'scroll-stats':
              this.stats.tileCommandCounts.scrollStats += 1;
              this.stats.recordHostScrollStats(
                cmd.scrollBatchesTotal,
                cmd.scrollFullFallbacksTotal,
                cmd.scrollPotentialTilesTotal,
                cmd.scrollSavedTilesTotal,
              );
              break;
            default:
              this.stats.tileCommandCounts.unknown += 1;
              break;
          }
          if (cmd.type === 'scroll-copy') {
            // During scroll-copy/grid-shift, stale video overlays drift from
            // true content position. Clear and wait for fresh video frames.
            this.clearVideoOverlay();
          } else if (cmd.type === 'grid-offset') {
            if (cmd.offsetX !== 0 || cmd.offsetY !== 0) {
              this.clearVideoOverlay();
            }
          } else if (cmd.type === 'grid-config') {
            this.clearVideoOverlay();
          }
        }
        break;
    }
  }

  private handleVideoFrame(payload: Uint8Array): void {
    const result = this.nalReassembler.push(payload);
    if (!result) return;
    this.decodeVideoNal(result.data, result.isKeyframe, result.tileInfo);
  }

  // ── Cursor ─────────────────────────────────────────────────────────

  private handleCursorUpdate(payload: Uint8Array): void {
    if (this.cursorRuntime.handlePayload(payload)) {
      this.displayDirty = true;
    }
  }

  // ── Clipboard ──────────────────────────────────────────────────────

  private handleClipboardUpdate(payload: Uint8Array): void {
    if (!this.options.clipboard || this.viewerRestricted) return;
    if (payload.length > 5 && payload[0] === 0x01) {
      const len = (payload[1] | (payload[2] << 8) | (payload[3] << 16) | (payload[4] << 24)) >>> 0;
      if (payload.length < 5 + len) return;
      const text = new TextDecoder().decode(payload.subarray(5, 5 + len));
      if (this.input) this.input.setLastClipboardHash(fnvHash(text));
      navigator.clipboard.writeText(text).catch(() => {});
    }
  }

  // ── Control ────────────────────────────────────────────────────────

  private handleControlMessage(payload: Uint8Array): void {
    this.controlRuntime.handle(payload);
  }

  // ── Resize ─────────────────────────────────────────────────────────

  private applyClientAccessState(flags: number, width: number, height: number): void {
    this.viewerRestricted = (flags & 0x01) !== 0;
    this.resizeRuntime.applyClientAccessState(flags, width, height);
    this.updateCapabilities();
  }

  private updateCapabilities(): void {
    this.capabilities = this.capabilityRuntime.apply({
      current: this.capabilities,
      sessionFlags: this.sessionFlags,
      microphoneEncoderSupported: this.microphoneEncoderSupported,
      cameraEncoderSupported: this.cameraEncoderSupported,
      viewerRestricted: this.viewerRestricted,
    });
  }

  private sendResizeRequest(width: number, height: number): void {
    const payload = new Uint8Array(5);
    payload[0] = 0x01; // CTRL_RESOLUTION_REQUEST
    payload[1] = width & 0xFF;
    payload[2] = (width >> 8) & 0xFF;
    payload[3] = height & 0xFF;
    payload[4] = (height >> 8) & 0xFF;
    this.sendFrame(CH_CONTROL, payload);
  }

  // ── Frame sending ──────────────────────────────────────────────────

  private sendTileCacheMiss(frameSeq: number, col: number, row: number, hash: bigint): void {
    const payload = new Uint8Array(17);
    const view = new DataView(payload.buffer);
    payload[0] = TILE_CACHE_MISS;
    view.setUint32(1, frameSeq >>> 0, true);
    view.setUint16(5, col & 0xFFFF, true);
    view.setUint16(7, row & 0xFFFF, true);
    view.setBigUint64(9, hash, true);
    this.sendFrame(CH_TILES, payload);
  }

  private sendCameraFrame(payload: Uint8Array): 'sent' | 'queued' | 'replaced' {
    if (payload.length === 0) {
      this.pendingCameraFrame = null;
      this.sendFrame(CH_VIDEO_IN, payload);
      return 'sent';
    }

    const frame = encodeFrame(CH_VIDEO_IN, payload);
    if (!this.sendWritable) {
      const replaced = this.pendingCameraFrame !== null;
      this.pendingCameraFrame = frame;
      return replaced ? 'replaced' : 'queued';
    }

    const ds = this.sendWritable.desiredSize;
    if (ds !== null && ds <= 0) {
      const replaced = this.pendingCameraFrame !== null;
      this.pendingCameraFrame = frame;
      this.schedulePendingCameraFlush();
      return replaced ? 'replaced' : 'queued';
    }

    this.stats.recordTx(CH_VIDEO_IN, frame.length);
    this.sendWritable.write(frame).catch((e) => {
      console.error('[bpane] camera frame write failed', e);
    });
    return 'sent';
  }

  private schedulePendingCameraFlush(delayMs = 40): void {
    if (this.pendingCameraFlushTimer) return;
    this.pendingCameraFlushTimer = setTimeout(() => {
      this.pendingCameraFlushTimer = null;
      this.flushPendingCameraFrame();
    }, delayMs);
  }

  private flushPendingCameraFrame(): void {
    if (!this.pendingCameraFrame || !this.sendWritable) return;

    const ds = this.sendWritable.desiredSize;
    if (ds !== null && ds <= 0) {
      this.schedulePendingCameraFlush();
      return;
    }

    const frame = this.pendingCameraFrame;
    this.pendingCameraFrame = null;
    this.stats.recordTx(CH_VIDEO_IN, frame.length);
    this.sendWritable.write(frame).catch((e) => {
      console.error('[bpane] pending camera frame write failed', e);
    });
    if (this.pendingCameraFrame) {
      this.schedulePendingCameraFlush(0);
    }
  }

  private sendFrame(channelId: number, payload: Uint8Array): void {
    if (this.isViewerBlockedChannel(channelId, payload)) {
      return;
    }

    const frame = encodeFrame(channelId, payload);

    if (!this.sendWritable) {
      this.pendingFrames.push(frame);
      return;
    }

    // Backpressure: drop low-priority frames when the write queue is full.
    // desiredSize <= 0 means the internal buffer is at or over capacity.
    const ds = this.sendWritable.desiredSize;
    if (ds !== null && ds <= 0 && channelId === CH_INPUT && payload[0] === INPUT_MOUSE_MOVE) {
      // Only mouse-move frames are safe to drop under pressure because the
      // next move supersedes the previous position. Keyboard/button frames
      // must preserve both press and release ordering.
      return;
    }
    if (ds !== null && ds <= 0 && channelId === CH_VIDEO_IN && payload.length > 0) {
      // Webcam ingress is live media; when transport backpressure builds,
      // drop stale frames instead of increasing end-to-end latency.
      return;
    }

    this.stats.recordTx(channelId, frame.length);
    this.sendWritable.write(frame).catch((e) => {
      console.error('[bpane] sendFrame write failed', e);
    });
  }

  private isViewerBlockedChannel(channelId: number, payload: Uint8Array): boolean {
    if (!this.viewerRestricted) {
      return false;
    }

    if (channelId === CH_CONTROL) {
      return payload[0] === CTRL_KEYBOARD_LAYOUT;
    }

    return channelId === CH_INPUT
      || channelId === CH_CLIPBOARD
      || channelId === CH_AUDIO_IN
      || channelId === CH_VIDEO_IN
      || channelId === CH_FILE_UP;
  }

  // ── Microphone (delegated to AudioController) ──────────────────────

  async startMicrophone(): Promise<void> {
    if (this.options.microphone === false) {
      throw new Error('microphone input is disabled in session options');
    }
    if (this.viewerRestricted) {
      throw new Error('microphone input is disabled for viewer sessions');
    }
    if (!this.microphoneEncoderSupported) {
      throw new Error('browser Opus microphone encoding is unavailable');
    }
    if (this.microphoneSupported === false) {
      throw new Error('microphone input is not supported by the host session');
    }
    return this.audio.startMicrophone();
  }

  stopMicrophone(): void {
    this.audio.stopMicrophone();
  }

  async startCamera(): Promise<void> {
    if (this.options.camera === false) {
      throw new Error('camera input is disabled in session options');
    }
    if (this.viewerRestricted) {
      throw new Error('camera input is disabled for viewer sessions');
    }
    if (!this.cameraEncoderSupported) {
      throw new Error('camera video encoding is not supported in this browser');
    }
    if (this.cameraSupported === false) {
      throw new Error('camera input is not supported by the host session');
    }
    return this.camera.startCamera();
  }

  stopCamera(): void {
    this.camera.stopCamera();
  }

  promptFileUpload(): void {
    if (this.viewerRestricted || !this.capabilities.fileTransfer) return;
    this.fileTransfer?.promptUpload();
  }

  async uploadFiles(files: FileList | Iterable<File>): Promise<void> {
    if (this.viewerRestricted) {
      throw new Error('file upload is disabled for viewer sessions');
    }
    if (!this.fileTransfer) return;
    await this.fileTransfer.uploadFiles(files);
  }
}

// Re-export layout helpers from input-controller.
export { inferLayoutName, inferLayoutHint, InputController } from './input-controller.js';
export type { InputControllerDeps } from './input-controller.js';

// Re-export utilities for external use
export { fnvHash } from './hash.js';
export { domCodeToEvdev, buildModifiers, normalizeScroll, createScrollState } from './input-map.js';
export { NalReassembler, parseTileInfo, getNalType } from './nal.js';
export { encodeFrame, parseFrames } from './protocol.js';
export type { TileInfo, ReassembledNal } from './nal.js';
export type { ScrollState } from './input-map.js';
export type { ParsedFrame } from './protocol.js';
export { TileCompositor } from './tile-compositor.js';
export { TileCache, parseTileMessage, CH_TILES } from './tile-cache.js';
export type { TileCommand, TileGridConfig } from './tile-cache.js';
