import { CH_CONTROL, CH_VIDEO } from './protocol.js';
import { AudioController } from './audio-controller.js';
import { CameraController } from './camera-controller.js';
import { FileTransferController } from './file-transfer.js';
import { InputController } from './input-controller.js';
import { SessionCapabilityRuntime } from './session-capability-runtime.js';
import { SessionControlRuntime } from './session-control-runtime.js';
import { SessionFrameRouterRuntime } from './session-frame-router-runtime.js';
import { SessionSendRuntime } from './session-send-runtime.js';
import { SessionStats } from './session-stats.js';
import { SessionStreamReaderRuntime } from './session-stream-reader-runtime.js';
import { SessionSurfaceRuntime } from './session-surface-runtime.js';
import { SessionTransportRuntime } from './session-transport-runtime.js';
import { SessionVideoDecoderRuntime } from './session-video-decoder-runtime.js';
import { TileCompositor } from './tile-compositor.js';

type SendFrame = (channelId: number, payload: Uint8Array) => void;
type SendCameraFrame = (payload: Uint8Array) => 'sent' | 'queued' | 'replaced';

interface SessionRuntimeFactoryOptions {
  audioEnabled: boolean;
  fileTransferEnabled: boolean;
  hiDpi: boolean;
  pingIntervalMs: number;
  renderBackend?: 'auto' | 'canvas2d' | 'webgl2';
  onCapabilitiesChange?: (capabilities: {
    audio: boolean;
    microphone: boolean;
    camera: boolean;
    fileTransfer: boolean;
    keyboardLayout: boolean;
  }) => void;
}

export interface SessionRuntimeFactoryContext {
  isConnected(): boolean;
  isViewerRestricted(): boolean;
  getInputController(): InputController | null;
  setRemoteSize(width: number, height: number): void;
  onResolutionChange(width: number, height: number): void;
  setSessionFlags(flags: number): void;
  setMicrophoneSupported(supported: boolean): void;
  setCameraSupported(supported: boolean): void;
  updateCapabilities(): void;
  applyClientAccessState(flags: number, width: number, height: number): void;
  handleVideoFrame(payload: Uint8Array): void;
  handleCursorUpdate(payload: Uint8Array): void;
  handleClipboardUpdate(payload: Uint8Array): void;
  handleControlMessage(payload: Uint8Array): void;
  clearVideoOverlay(): void;
  onConnect(): void;
  onDisconnect(reason: string): void;
  onError(error: Error): void;
  handleStream(stream: WebTransportBidirectionalStream): Promise<void>;
  sendPing(): void;
  sendResizeRequest(width: number, height: number): void;
  sendTileCacheMiss(frameSeq: number, col: number, row: number, hash: bigint): void;
  sendFrame: SendFrame;
  sendCameraFrame: SendCameraFrame;
}

export interface SessionRuntimeFactoryParams {
  container: HTMLElement;
  tileCompositor: TileCompositor;
  stats: SessionStats;
  options: SessionRuntimeFactoryOptions;
  context: SessionRuntimeFactoryContext;
}

export interface SessionRuntimeBundle {
  audio: AudioController;
  camera: CameraController;
  fileTransfer: FileTransferController;
  capabilityRuntime: SessionCapabilityRuntime;
  controlRuntime: SessionControlRuntime;
  sendRuntime: SessionSendRuntime;
  streamReaderRuntime: SessionStreamReaderRuntime;
  frameRouterRuntime: SessionFrameRouterRuntime;
  transportRuntime: SessionTransportRuntime;
  surfaceRuntime: SessionSurfaceRuntime;
  videoDecoderRuntime: SessionVideoDecoderRuntime;
}

interface SessionRuntimeFactoryDeps {
  createAudioController(enabled: boolean, sendFrame: SendFrame): AudioController;
  createCameraController(sendCameraFrame: SendCameraFrame): CameraController;
  createFileTransferController(
    options: ConstructorParameters<typeof FileTransferController>[0],
  ): FileTransferController;
  createSessionCapabilityRuntime(
    options: ConstructorParameters<typeof SessionCapabilityRuntime>[0],
  ): SessionCapabilityRuntime;
  createSessionControlRuntime(
    options: ConstructorParameters<typeof SessionControlRuntime>[0],
  ): SessionControlRuntime;
  createSessionSendRuntime(
    options: ConstructorParameters<typeof SessionSendRuntime>[0],
  ): SessionSendRuntime;
  createSessionFrameRouterRuntime(
    options: ConstructorParameters<typeof SessionFrameRouterRuntime>[0],
  ): SessionFrameRouterRuntime;
  createSessionStreamReaderRuntime(
    options: ConstructorParameters<typeof SessionStreamReaderRuntime>[0],
  ): SessionStreamReaderRuntime;
  createSessionSurfaceRuntime(
    options: ConstructorParameters<typeof SessionSurfaceRuntime>[0],
  ): SessionSurfaceRuntime;
  createSessionTransportRuntime(
    options: ConstructorParameters<typeof SessionTransportRuntime>[0],
  ): SessionTransportRuntime;
  createSessionVideoDecoderRuntime(
    options: ConstructorParameters<typeof SessionVideoDecoderRuntime>[0],
  ): SessionVideoDecoderRuntime;
}

export class SessionRuntimeFactory {
  private readonly deps: SessionRuntimeFactoryDeps;

  constructor(overrides: Partial<SessionRuntimeFactoryDeps> = {}) {
    this.deps = {
      createAudioController: overrides.createAudioController
        ?? ((enabled, sendFrame) => new AudioController(enabled, sendFrame)),
      createCameraController: overrides.createCameraController
        ?? ((sendCameraFrame) => new CameraController(sendCameraFrame)),
      createFileTransferController: overrides.createFileTransferController
        ?? ((options) => new FileTransferController(options)),
      createSessionCapabilityRuntime: overrides.createSessionCapabilityRuntime
        ?? ((options) => new SessionCapabilityRuntime(options)),
      createSessionControlRuntime: overrides.createSessionControlRuntime
        ?? ((options) => new SessionControlRuntime(options)),
      createSessionSendRuntime: overrides.createSessionSendRuntime
        ?? ((options) => new SessionSendRuntime(options)),
      createSessionFrameRouterRuntime: overrides.createSessionFrameRouterRuntime
        ?? ((options) => new SessionFrameRouterRuntime(options)),
      createSessionStreamReaderRuntime: overrides.createSessionStreamReaderRuntime
        ?? ((options) => new SessionStreamReaderRuntime(options)),
      createSessionSurfaceRuntime: overrides.createSessionSurfaceRuntime
        ?? ((options) => new SessionSurfaceRuntime(options)),
      createSessionTransportRuntime: overrides.createSessionTransportRuntime
        ?? ((options) => new SessionTransportRuntime(options)),
      createSessionVideoDecoderRuntime: overrides.createSessionVideoDecoderRuntime
        ?? ((options) => new SessionVideoDecoderRuntime(options)),
    };
  }

  create(params: SessionRuntimeFactoryParams): SessionRuntimeBundle {
    const audio = this.deps.createAudioController(
      params.options.audioEnabled,
      params.context.sendFrame,
    );
    const camera = this.deps.createCameraController(params.context.sendCameraFrame);
    const fileTransfer = this.deps.createFileTransferController({
      container: params.container,
      enabled: params.options.fileTransferEnabled,
      sendFrame: params.context.sendFrame,
    });
    const capabilityRuntime = this.deps.createSessionCapabilityRuntime({
      fileTransferOptionEnabled: params.options.fileTransferEnabled,
      stopMicrophone: () => {
        audio.stopMicrophone();
      },
      stopCamera: () => {
        camera.stopCamera();
      },
      setFileTransferEnabled: (enabled) => {
        fileTransfer.setEnabled(enabled);
      },
      onCapabilitiesChange: (capabilities) => {
        params.options.onCapabilitiesChange?.(capabilities);
      },
    });
    const controlRuntime = this.deps.createSessionControlRuntime({
      setRemoteSize: params.context.setRemoteSize,
      onResolutionChange: params.context.onResolutionChange,
      setSessionFlags: params.context.setSessionFlags,
      setMicrophoneSupported: params.context.setMicrophoneSupported,
      setCameraSupported: params.context.setCameraSupported,
      configureInputExtendedKeyEvents: (enabled) => {
        const input = params.context.getInputController();
        if (input) {
          input.serverSupportsKeyEventEx = enabled;
        }
      },
      sendLayoutHint: () => {
        params.context.getInputController()?.sendLayoutHint();
      },
      updateCapabilities: params.context.updateCapabilities,
      applyClientAccessState: params.context.applyClientAccessState,
      sendControlFrame: (payload) => {
        params.context.sendFrame(CH_CONTROL, payload);
      },
    });
    const sendRuntime = this.deps.createSessionSendRuntime({
      isViewerRestricted: params.context.isViewerRestricted,
      recordTx: (channelId, bytes) => {
        params.stats.recordTx(channelId, bytes);
      },
      onWriteError: (message, error) => {
        console.error(message, error);
      },
    });
    const surfaceRuntime = this.deps.createSessionSurfaceRuntime({
      container: params.container,
      tileCompositor: params.tileCompositor,
      hiDpi: params.options.hiDpi,
      renderBackend: params.options.renderBackend,
      onTileCacheMiss: ({ frameSeq, col, row, hash }) => {
        params.context.sendTileCacheMiss(frameSeq, col, row, hash);
      },
      sendResizeRequest: params.context.sendResizeRequest,
      setRemoteSize: params.context.setRemoteSize,
      onResolutionChange: params.context.onResolutionChange,
    });
    const frameRouterRuntime = this.deps.createSessionFrameRouterRuntime({
      tileCompositor: params.tileCompositor,
      stats: params.stats,
      handleVideoFrame: params.context.handleVideoFrame,
      handleAudioFrame: (payload) => {
        audio.handleFrame(payload);
      },
      handleCursorUpdate: params.context.handleCursorUpdate,
      handleClipboardUpdate: params.context.handleClipboardUpdate,
      handleControlMessage: params.context.handleControlMessage,
      handleFileDownloadFrame: (payload) => {
        fileTransfer.handleFrame(payload);
      },
      clearVideoOverlay: params.context.clearVideoOverlay,
      markDisplayDirty: () => {
        surfaceRuntime.markDisplayDirty();
      },
    });
    const streamReaderRuntime = this.deps.createSessionStreamReaderRuntime({
      isConnected: params.context.isConnected,
      recordRx: (channelId, bytes) => {
        params.stats.recordRx(channelId, bytes);
      },
      onFrame: (channelId, payload) => {
        frameRouterRuntime.handleFrame(channelId, payload);
      },
      onReadError: (error) => {
        console.error('[bpane] stream read error:', error);
      },
    });
    const transportRuntime = this.deps.createSessionTransportRuntime({
      onConnect: params.context.onConnect,
      onDisconnect: params.context.onDisconnect,
      onError: params.context.onError,
      onStream: params.context.handleStream,
      onDatagram: (datagram) => {
        params.stats.recordRx(CH_VIDEO, datagram.byteLength);
        params.stats.videoDatagramsRx += 1;
        params.stats.videoDatagramBytesRx += datagram.byteLength;
        params.context.handleVideoFrame(datagram);
      },
      onDatagramReadError: (error) => {
        console.error('[bpane] datagram read error:', error);
      },
      sendPing: params.context.sendPing,
      pingIntervalMs: params.options.pingIntervalMs,
    });
    const videoDecoderRuntime = this.deps.createSessionVideoDecoderRuntime({
      onDecodedFrame: (frame, tileInfo) => {
        surfaceRuntime.handleDecodedFrame(frame, tileInfo);
      },
      incrementFrameCount: () => {
        params.stats.frameCount += 1;
      },
      incrementDroppedFrame: () => {
        params.stats.videoFramesDropped += 1;
      },
      onDecoderError: (error) => {
        console.error('[bpane] VideoDecoder error:', error.message);
      },
    });

    return {
      audio,
      camera,
      fileTransfer,
      capabilityRuntime,
      controlRuntime,
      sendRuntime,
      streamReaderRuntime,
      frameRouterRuntime,
      transportRuntime,
      surfaceRuntime,
      videoDecoderRuntime,
    };
  }
}
