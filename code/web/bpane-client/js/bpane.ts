/**
 * BrowserPane Client - Public TypeScript API
 *
 * Embeds a remote Linux desktop into any HTML container element.
 * This is the single source of truth for all browser-side BrowserPane logic.
 */

import {
  CH_VIDEO,
  CH_CONTROL,
} from './protocol.js';

import { NalReassembler } from './nal.js';
import { fnvHash } from './hash.js';
import { TileCompositor, CH_TILES } from './tile-compositor.js';
import type { WebGLRendererDiagnostics } from './webgl-compositor.js';
import { SessionStats } from './session-stats.js';
import { AudioController } from './audio-controller.js';
import { CameraController } from './camera-controller.js';
import { FileTransferController } from './file-transfer.js';
import { InputController } from './input-controller.js';
import { SessionCapabilityRuntime } from './session-capability-runtime.js';
import { SessionControlRuntime } from './session-control-runtime.js';
import { SessionFrameRouterRuntime } from './session-frame-router-runtime.js';
import { SessionSendRuntime } from './session-send-runtime.js';
import { SessionStreamReaderRuntime } from './session-stream-reader-runtime.js';
import { SessionSurfaceRuntime } from './session-surface-runtime.js';
import { SessionTransportRuntime } from './session-transport-runtime.js';
import { SessionVideoDecoderRuntime } from './session-video-decoder-runtime.js';
import { SessionConnectOptionsValidator } from './shared/connect-options-validator.js';

export type RenderBackendPreference = 'auto' | 'canvas2d' | 'webgl2';

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
  /** Diagnostic backend override. Defaults to 'auto'. */
  renderBackend?: RenderBackendPreference;
  /** Diagnostic switch. Disable retained scroll-copy reuse and rely on repair tiles only. */
  scrollCopy?: boolean;
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
export class BpaneSession {
  private container: HTMLElement;
  private options: BpaneOptions;
  private connected = false;
  // Extracted: input event handling
  private input: InputController | null = null;
  private sendRuntime: SessionSendRuntime;
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
  private frameRouterRuntime: SessionFrameRouterRuntime;
  private streamReaderRuntime: SessionStreamReaderRuntime;
  private transportRuntime: SessionTransportRuntime;
  private pingSeq = 0;
  private remoteWidth = 0;
  private remoteHeight = 0;
  // Gateway-managed access state.
  private viewerRestricted = false;
  private controlRuntime: SessionControlRuntime;

  // Tile compositor
  private tileCompositor = new TileCompositor();

  // NAL reassembly
  private nalReassembler = new NalReassembler();

  // Render surface/runtime
  private surfaceRuntime: SessionSurfaceRuntime;
  private videoDecoderRuntime: SessionVideoDecoderRuntime;
  // Extracted: all transfer/tile/scroll stats live in SessionStats
  private stats = new SessionStats();

  private constructor(options: BpaneOptions) {
    this.options = options;
    this.container = options.container;
    this.tileCompositor.setScrollCopyEnabled(options.scrollCopy !== false);
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
    this.sendRuntime = new SessionSendRuntime({
      isViewerRestricted: () => this.viewerRestricted,
      recordTx: (channelId, bytes) => {
        this.stats.recordTx(channelId, bytes);
      },
      onWriteError: (message, error) => {
        console.error(message, error);
      },
    });
    this.streamReaderRuntime = new SessionStreamReaderRuntime({
      isConnected: () => this.connected,
      recordRx: (channelId, bytes) => {
        this.stats.recordRx(channelId, bytes);
      },
      onFrame: (channelId, payload) => {
        this.frameRouterRuntime.handleFrame(channelId, payload);
      },
      onReadError: (error) => {
        console.error('[bpane] stream read error:', error);
      },
    });
    this.frameRouterRuntime = new SessionFrameRouterRuntime({
      tileCompositor: this.tileCompositor,
      stats: this.stats,
      handleVideoFrame: (payload) => {
        this.handleVideoFrame(payload);
      },
      handleAudioFrame: (payload) => {
        this.audio.handleFrame(payload);
      },
      handleCursorUpdate: (payload) => {
        this.handleCursorUpdate(payload);
      },
      handleClipboardUpdate: (payload) => {
        this.handleClipboardUpdate(payload);
      },
      handleControlMessage: (payload) => {
        this.handleControlMessage(payload);
      },
      handleFileDownloadFrame: (payload) => {
        this.fileTransfer?.handleFrame(payload);
      },
      clearVideoOverlay: () => {
        this.clearVideoOverlay();
      },
      markDisplayDirty: () => {
        this.surfaceRuntime.markDisplayDirty();
      },
    });
    this.transportRuntime = new SessionTransportRuntime({
      onConnect: () => {
        this.connected = true;
        this.options.onConnect?.();
      },
      onDisconnect: (reason) => {
        if (!this.connected) {
          return;
        }
        this.connected = false;
        this.options.onDisconnect?.(reason);
      },
      onError: (error) => {
        this.options.onError?.(error);
      },
      onStream: (stream) => this.handleStream(stream),
      onDatagram: (datagram) => {
        this.stats.recordRx(CH_VIDEO, datagram.byteLength);
        this.stats.videoDatagramsRx += 1;
        this.stats.videoDatagramBytesRx += datagram.byteLength;
        this.handleVideoFrame(datagram);
      },
      onDatagramReadError: (error) => {
        console.error('[bpane] datagram read error:', error);
      },
      sendPing: () => {
        this.sendPing();
      },
      pingIntervalMs: PING_INTERVAL_MS,
    });

    this.surfaceRuntime = new SessionSurfaceRuntime({
      container: this.container,
      tileCompositor: this.tileCompositor,
      hiDpi: options.hiDpi ?? false,
      renderBackend: options.renderBackend,
      onTileCacheMiss: ({ frameSeq, col, row, hash }) => {
        this.sendTileCacheMiss(frameSeq, col, row, hash);
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
    this.videoDecoderRuntime = new SessionVideoDecoderRuntime({
      onDecodedFrame: (frame, tileInfo) => {
        this.surfaceRuntime.handleDecodedFrame(frame, tileInfo);
      },
      incrementFrameCount: () => {
        this.stats.frameCount += 1;
      },
      incrementDroppedFrame: () => {
        this.stats.videoFramesDropped += 1;
      },
      onDecoderError: (error) => {
        console.error('[bpane] VideoDecoder error:', error.message);
      },
    });
    this.surfaceRuntime.start();
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
      canvas: session.surfaceRuntime.getCanvas(),
      sendFrame: (channelId, payload) => session.sendFrame(channelId, payload),
      drawCursor: (_shape, x, y) => session.surfaceRuntime.drawCursorMove(x, y),
      getRemoteDims: () => ({
        width: session.remoteWidth || session.surfaceRuntime.getCanvas().width,
        height: session.remoteHeight || session.surfaceRuntime.getCanvas().height,
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
    return this.surfaceRuntime.getRenderDiagnostics();
  }

  /**
   * Disconnect and clean up.
   */
  disconnect(): void {
    if (!this.connected) return;
    this.connected = false;

    // Remove all DOM event listeners
    if (this.input) {
      this.input.destroy();
      this.input = null;
    }

    this.videoDecoderRuntime.destroy();

    this.camera.destroy();
    this.audio.destroy();
    if (this.fileTransfer) {
      this.fileTransfer.destroy();
      this.fileTransfer = null;
    }

    this.transportRuntime.disconnect();
    this.surfaceRuntime.destroy();

    this.tileCompositor.reset();
    this.sendRuntime.destroy();
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
    this.surfaceRuntime.clearVideoOverlay();
  }

  private async setupTransport(): Promise<void> {
    await this.transportRuntime.connect({
      gatewayUrl: this.options.gatewayUrl,
      token: this.options.token,
      certHashUrl: this.options.certHashUrl,
    });
  }

  // ── Video decode via WebCodecs ────────────────────────────────────

  // ── Stream handling ────────────────────────────────────────────────

  private async handleStream(stream: WebTransportBidirectionalStream): Promise<void> {
    if (!this.sendRuntime.hasWriter()) {
      this.sendRuntime.attachWriter(stream.writable.getWriter(), () => {
        const dims = this.surfaceRuntime.getContainerResizeDims();
        this.sendResizeRequest(dims.width, dims.height);
      });
    }
    await this.streamReaderRuntime.readStream(stream);
  }

  private handleVideoFrame(payload: Uint8Array): void {
    const result = this.nalReassembler.push(payload);
    if (!result) return;
    this.videoDecoderRuntime.decodeNal(result.data, result.tileInfo);
  }

  // ── Cursor ─────────────────────────────────────────────────────────

  private handleCursorUpdate(payload: Uint8Array): void {
    this.surfaceRuntime.handleCursorPayload(payload);
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
    this.surfaceRuntime.applyClientAccessState(flags, width, height);
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

  private sendPing(): void {
    this.pingSeq++;
    const now = BigInt(Date.now());
    const payload = new Uint8Array(13);
    const view = new DataView(payload.buffer);
    payload[0] = 0x04; // Ping tag
    view.setUint32(1, this.pingSeq, true);
    view.setUint32(5, Number(now & 0xFFFFFFFFn), true);
    view.setUint32(9, Number((now >> 32n) & 0xFFFFFFFFn), true);
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
    return this.sendRuntime.sendCameraFrame(payload);
  }

  private sendFrame(channelId: number, payload: Uint8Array): void {
    this.sendRuntime.sendFrame(channelId, payload);
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
