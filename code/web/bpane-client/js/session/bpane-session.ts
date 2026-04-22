import {
  CH_VIDEO,
  CH_CONTROL,
} from '../protocol.js';
import { AudioController } from '../audio-controller.js';
import { CameraController } from '../camera-controller.js';
import { FileTransferController } from '../file-transfer.js';
import { fnvHash } from '../hash.js';
import { InputController } from '../input-controller.js';
import { NalReassembler } from '../nal.js';
import type { BpaneOptions, SessionCapabilities } from '../bpane-types.js';
import { SessionCapabilityRuntime } from '../session-capability-runtime.js';
import { SessionControlRuntime } from '../session-control-runtime.js';
import { SessionFrameRouterRuntime } from '../session-frame-router-runtime.js';
import { SessionRuntimeFactory } from '../session-runtime-factory.js';
import { SessionSendRuntime } from '../session-send-runtime.js';
import { SessionStats } from '../session-stats.js';
import { SessionStreamReaderRuntime } from '../session-stream-reader-runtime.js';
import { SessionSurfaceRuntime } from '../session-surface-runtime.js';
import { SessionTransportRuntime } from '../session-transport-runtime.js';
import { SessionVideoDecoderRuntime } from '../session-video-decoder-runtime.js';
import { SessionConnectOptionsValidator } from '../shared/connect-options-validator.js';
import { TileCompositor, CH_TILES } from '../tile-compositor.js';
import type { WebGLRendererDiagnostics } from '../webgl-compositor.js';

const PING_INTERVAL_MS = 5000;
const TILE_CACHE_MISS = 0x09;

export class BpaneSession {
  private container: HTMLElement;
  private options: BpaneOptions;
  private connected = false;
  private input: InputController | null = null;
  private sendRuntime: SessionSendRuntime;
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
  private viewerRestricted = false;
  private controlRuntime: SessionControlRuntime;
  private tileCompositor = new TileCompositor();
  private nalReassembler = new NalReassembler();
  private surfaceRuntime: SessionSurfaceRuntime;
  private videoDecoderRuntime: SessionVideoDecoderRuntime;
  private stats = new SessionStats();

  private constructor(options: BpaneOptions) {
    this.options = options;
    this.container = options.container;
    this.tileCompositor.setScrollCopyEnabled(options.scrollCopy !== false);
    const runtimes = new SessionRuntimeFactory().create({
      container: this.container,
      tileCompositor: this.tileCompositor,
      stats: this.stats,
      options: {
        audioEnabled: options.audio ?? true,
        fileTransferEnabled: options.fileTransfer !== false,
        hiDpi: options.hiDpi ?? false,
        pingIntervalMs: PING_INTERVAL_MS,
        renderBackend: options.renderBackend,
        onCapabilitiesChange: (capabilities) => {
          this.options.onCapabilitiesChange?.(capabilities);
        },
      },
      context: {
        isConnected: () => this.connected,
        isViewerRestricted: () => this.viewerRestricted,
        getInputController: () => this.input,
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
        updateCapabilities: () => {
          this.updateCapabilities();
        },
        applyClientAccessState: (flags, width, height) => {
          this.applyClientAccessState(flags, width, height);
        },
        handleVideoFrame: (payload) => {
          this.handleVideoFrame(payload);
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
        clearVideoOverlay: () => {
          this.clearVideoOverlay();
        },
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
        handleStream: (stream) => this.handleStream(stream),
        sendPing: () => {
          this.sendPing();
        },
        sendResizeRequest: (width, height) => {
          this.sendResizeRequest(width, height);
        },
        sendTileCacheMiss: (frameSeq, col, row, hash) => {
          this.sendTileCacheMiss(frameSeq, col, row, hash);
        },
        sendFrame: (channelId, payload) => {
          this.sendFrame(channelId, payload);
        },
        sendCameraFrame: (payload) => this.sendCameraFrame(payload),
      },
    });
    this.audio = runtimes.audio;
    this.camera = runtimes.camera;
    this.fileTransfer = runtimes.fileTransfer;
    this.capabilityRuntime = runtimes.capabilityRuntime;
    this.controlRuntime = runtimes.controlRuntime;
    this.sendRuntime = runtimes.sendRuntime;
    this.streamReaderRuntime = runtimes.streamReaderRuntime;
    this.frameRouterRuntime = runtimes.frameRouterRuntime;
    this.transportRuntime = runtimes.transportRuntime;
    this.surfaceRuntime = runtimes.surfaceRuntime;
    this.videoDecoderRuntime = runtimes.videoDecoderRuntime;
    this.surfaceRuntime.start();
  }

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

  getFrameCount(): number { return this.stats.frameCount; }

  getTileCacheStats(): {
    hits: number;
    misses: number;
    hitRate: number;
    size: number;
    bytes: number;
    evictions: number;
    fills: number;
    qoiDecodes: number;
    qoiRedundant: number;
    qoiRedundantBytes: number;
    zstdDecodes: number;
    zstdRedundant: number;
    zstdRedundantBytes: number;
    cacheMisses: number;
    scrollCopies: number;
    batchesQueued: number;
    totalBatchCommands: number;
    maxBatchCommands: number;
    lastBatchCommands: number;
    currentPendingCommands: number;
    pendingCommandsHighWaterMark: number;
  } {
    const cache = this.tileCompositor.getCache();
    const stats = this.tileCompositor.stats;
    return {
      hits: cache.hits,
      misses: cache.misses,
      hitRate: cache.hitRate,
      size: cache.size,
      bytes: cache.bytes,
      evictions: cache.evictions,
      fills: stats.fills,
      qoiDecodes: stats.qoiDecodes,
      qoiRedundant: stats.qoiRedundant,
      qoiRedundantBytes: stats.qoiRedundantBytes,
      zstdDecodes: stats.zstdDecodes,
      zstdRedundant: stats.zstdRedundant,
      zstdRedundantBytes: stats.zstdRedundantBytes,
      cacheMisses: stats.cacheMisses,
      scrollCopies: stats.scrollCopies,
      batchesQueued: stats.batchesQueued,
      totalBatchCommands: stats.totalBatchCommands,
      maxBatchCommands: stats.maxBatchCommands,
      lastBatchCommands: stats.lastBatchCommands,
      currentPendingCommands: stats.currentPendingCommands,
      pendingCommandsHighWaterMark: stats.pendingCommandsHighWaterMark,
    };
  }

  getSessionStats(): import('../session-stats.js').SessionStatsSnapshot {
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

  getRenderDiagnostics(): WebGLRendererDiagnostics {
    return this.surfaceRuntime.getRenderDiagnostics();
  }

  disconnect(): void {
    if (!this.connected) return;
    this.connected = false;
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
      connectTicket: this.options.connectTicket,
      accessToken: this.options.accessToken ?? this.options.token,
      certHashUrl: this.options.certHashUrl,
    });
  }

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

  private handleCursorUpdate(payload: Uint8Array): void {
    this.surfaceRuntime.handleCursorPayload(payload);
  }

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

  private handleControlMessage(payload: Uint8Array): void {
    this.controlRuntime.handle(payload);
  }

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
    payload[0] = 0x01;
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
    payload[0] = 0x04;
    view.setUint32(1, this.pingSeq, true);
    view.setUint32(5, Number(now & 0xFFFFFFFFn), true);
    view.setUint32(9, Number((now >> 32n) & 0xFFFFFFFFn), true);
    this.sendFrame(CH_CONTROL, payload);
  }

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
