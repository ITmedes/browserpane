import { describe, expect, it, vi } from 'vitest';
import { CH_CONTROL, CH_VIDEO } from '../protocol.js';
import { SessionRuntimeFactory } from '../session-runtime-factory.js';
import { SessionStats } from '../session-stats.js';

function createContainer(): HTMLDivElement {
  return document.createElement('div');
}

describe('SessionRuntimeFactory', () => {
  it('creates the runtime graph and preserves the expected callback wiring', async () => {
    const container = createContainer();
    const tileCompositor = { id: 'tile-compositor' } as any;
    const stats = new SessionStats();
    const recordRxSpy = vi.spyOn(stats, 'recordRx');
    const recordTxSpy = vi.spyOn(stats, 'recordTx');
    const input = {
      serverSupportsKeyEventEx: false,
      sendLayoutHint: vi.fn(),
    } as any;
    const onCapabilitiesChange = vi.fn();
    const context = {
      isConnected: vi.fn(() => true),
      isViewerRestricted: vi.fn(() => false),
      getInputController: vi.fn(() => input),
      setRemoteSize: vi.fn(),
      onResolutionChange: vi.fn(),
      setSessionFlags: vi.fn(),
      setMicrophoneSupported: vi.fn(),
      setCameraSupported: vi.fn(),
      updateCapabilities: vi.fn(),
      applyClientAccessState: vi.fn(),
      handleVideoFrame: vi.fn(),
      handleCursorUpdate: vi.fn(),
      handleClipboardUpdate: vi.fn(),
      handleControlMessage: vi.fn(),
      clearVideoOverlay: vi.fn(),
      onConnect: vi.fn(),
      onDisconnect: vi.fn(),
      onError: vi.fn(),
      handleStream: vi.fn().mockResolvedValue(undefined),
      sendPing: vi.fn(),
      sendResizeRequest: vi.fn(),
      sendTileCacheMiss: vi.fn(),
      sendFrame: vi.fn(),
      sendCameraFrame: vi.fn(() => 'queued' as const),
    };

    const audio = { stopMicrophone: vi.fn(), destroy: vi.fn() } as any;
    const camera = { stopCamera: vi.fn(), destroy: vi.fn() } as any;
    const fileTransfer = {
      setEnabled: vi.fn(),
      handleFrame: vi.fn(),
      destroy: vi.fn(),
    } as any;
    const surfaceRuntime = {
      handleDecodedFrame: vi.fn(),
      markDisplayDirty: vi.fn(),
      clearVideoOverlay: vi.fn(),
      start: vi.fn(),
      destroy: vi.fn(),
    } as any;
    const frameRouterRuntime = { handleFrame: vi.fn() } as any;
    const controlRuntime = { handle: vi.fn() } as any;
    const sendRuntime = { sendFrame: vi.fn(), sendCameraFrame: vi.fn(), destroy: vi.fn() } as any;
    const streamReaderRuntime = { readStream: vi.fn() } as any;
    const transportRuntime = { connect: vi.fn(), disconnect: vi.fn() } as any;
    const capabilityRuntime = { apply: vi.fn() } as any;
    const videoDecoderRuntime = { decodeNal: vi.fn(), destroy: vi.fn() } as any;

    const captured: Record<string, any> = {};
    const factory = new SessionRuntimeFactory({
      createAudioController: vi.fn((enabled, sendFrame) => {
        captured.audio = { enabled, sendFrame };
        return audio;
      }),
      createCameraController: vi.fn((sendCameraFrame) => {
        captured.camera = { sendCameraFrame };
        return camera;
      }),
      createFileTransferController: vi.fn((options) => {
        captured.fileTransfer = options;
        return fileTransfer;
      }),
      createSessionCapabilityRuntime: vi.fn((options) => {
        captured.capability = options;
        return capabilityRuntime;
      }),
      createSessionControlRuntime: vi.fn((options) => {
        captured.control = options;
        return controlRuntime;
      }),
      createSessionSendRuntime: vi.fn((options) => {
        captured.send = options;
        return sendRuntime;
      }),
      createSessionSurfaceRuntime: vi.fn((options) => {
        captured.surface = options;
        return surfaceRuntime;
      }),
      createSessionFrameRouterRuntime: vi.fn((options) => {
        captured.frameRouter = options;
        return frameRouterRuntime;
      }),
      createSessionStreamReaderRuntime: vi.fn((options) => {
        captured.streamReader = options;
        return streamReaderRuntime;
      }),
      createSessionTransportRuntime: vi.fn((options) => {
        captured.transport = options;
        return transportRuntime;
      }),
      createSessionVideoDecoderRuntime: vi.fn((options) => {
        captured.videoDecoder = options;
        return videoDecoderRuntime;
      }),
    });

    const bundle = factory.create({
      container,
      tileCompositor,
      stats,
      options: {
        audioEnabled: true,
        fileTransferEnabled: true,
        hiDpi: false,
        pingIntervalMs: 5000,
        renderBackend: 'canvas2d',
        onCapabilitiesChange,
      },
      context,
    });

    expect(bundle).toEqual({
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
    });

    expect(captured.audio.enabled).toBe(true);
    expect(captured.fileTransfer.container).toBe(container);
    expect(captured.fileTransfer.enabled).toBe(true);
    expect(captured.surface.container).toBe(container);
    expect(captured.surface.tileCompositor).toBe(tileCompositor);
    expect(captured.surface.renderBackend).toBe('canvas2d');

    const framePayload = new Uint8Array([1, 2, 3]);
    captured.audio.sendFrame(7, framePayload);
    captured.camera.sendCameraFrame(framePayload);
    captured.fileTransfer.sendFrame(9, framePayload);
    expect(context.sendFrame).toHaveBeenCalledWith(7, framePayload);
    expect(context.sendCameraFrame).toHaveBeenCalledWith(framePayload);
    expect(context.sendFrame).toHaveBeenCalledWith(9, framePayload);

    captured.capability.stopMicrophone();
    captured.capability.stopCamera();
    captured.capability.setFileTransferEnabled(false);
    captured.capability.onCapabilitiesChange({
      audio: true,
      microphone: false,
      camera: false,
      fileTransfer: true,
      keyboardLayout: true,
    });
    expect(audio.stopMicrophone).toHaveBeenCalledOnce();
    expect(camera.stopCamera).toHaveBeenCalledOnce();
    expect(fileTransfer.setEnabled).toHaveBeenCalledWith(false);
    expect(onCapabilitiesChange).toHaveBeenCalledOnce();

    captured.control.setRemoteSize(1024, 768);
    captured.control.onResolutionChange(1024, 768);
    captured.control.setSessionFlags(7);
    captured.control.setMicrophoneSupported(true);
    captured.control.setCameraSupported(false);
    captured.control.configureInputExtendedKeyEvents(true);
    captured.control.sendLayoutHint();
    captured.control.updateCapabilities();
    captured.control.applyClientAccessState(1, 1024, 768);
    captured.control.sendControlFrame(framePayload);
    expect(context.setRemoteSize).toHaveBeenCalledWith(1024, 768);
    expect(context.onResolutionChange).toHaveBeenCalledWith(1024, 768);
    expect(context.setSessionFlags).toHaveBeenCalledWith(7);
    expect(context.setMicrophoneSupported).toHaveBeenCalledWith(true);
    expect(context.setCameraSupported).toHaveBeenCalledWith(false);
    expect(input.serverSupportsKeyEventEx).toBe(true);
    expect(input.sendLayoutHint).toHaveBeenCalledOnce();
    expect(context.updateCapabilities).toHaveBeenCalledOnce();
    expect(context.applyClientAccessState).toHaveBeenCalledWith(1, 1024, 768);
    expect(context.sendFrame).toHaveBeenCalledWith(CH_CONTROL, framePayload);

    captured.send.recordTx(4, 12);
    expect(recordTxSpy).toHaveBeenCalledWith(4, 12);

    captured.surface.onTileCacheMiss({ frameSeq: 5, col: 6, row: 7, hash: 8n });
    captured.surface.sendResizeRequest(1280, 720);
    captured.surface.setRemoteSize(1280, 720);
    captured.surface.onResolutionChange(1280, 720);
    expect(context.sendTileCacheMiss).toHaveBeenCalledWith(5, 6, 7, 8n);
    expect(context.sendResizeRequest).toHaveBeenCalledWith(1280, 720);
    expect(context.setRemoteSize).toHaveBeenCalledWith(1280, 720);
    expect(context.onResolutionChange).toHaveBeenCalledWith(1280, 720);

    captured.frameRouter.handleVideoFrame(framePayload);
    captured.frameRouter.handleCursorUpdate(framePayload);
    captured.frameRouter.handleClipboardUpdate(framePayload);
    captured.frameRouter.handleControlMessage(framePayload);
    captured.frameRouter.handleFileDownloadFrame(framePayload);
    captured.frameRouter.clearVideoOverlay();
    captured.frameRouter.markDisplayDirty();
    expect(context.handleVideoFrame).toHaveBeenCalledWith(framePayload);
    expect(context.handleCursorUpdate).toHaveBeenCalledWith(framePayload);
    expect(context.handleClipboardUpdate).toHaveBeenCalledWith(framePayload);
    expect(context.handleControlMessage).toHaveBeenCalledWith(framePayload);
    expect(fileTransfer.handleFrame).toHaveBeenCalledWith(framePayload);
    expect(context.clearVideoOverlay).toHaveBeenCalledOnce();
    expect(surfaceRuntime.markDisplayDirty).toHaveBeenCalledOnce();

    captured.streamReader.recordRx(10, 20);
    captured.streamReader.onFrame(11, framePayload);
    expect(recordRxSpy).toHaveBeenCalledWith(10, 20);
    expect(frameRouterRuntime.handleFrame).toHaveBeenCalledWith(11, framePayload);

    const bidiStream = {} as WebTransportBidirectionalStream;
    await captured.transport.onStream(bidiStream);
    captured.transport.onDatagram(framePayload);
    captured.transport.onConnect();
    captured.transport.onDisconnect('closed');
    captured.transport.onError(new Error('boom'));
    captured.transport.sendPing();
    expect(context.handleStream).toHaveBeenCalledWith(bidiStream);
    expect(recordRxSpy).toHaveBeenCalledWith(CH_VIDEO, framePayload.byteLength);
    expect(context.handleVideoFrame).toHaveBeenCalledWith(framePayload);
    expect(stats.videoDatagramsRx).toBe(1);
    expect(stats.videoDatagramBytesRx).toBe(framePayload.byteLength);
    expect(context.onConnect).toHaveBeenCalledOnce();
    expect(context.onDisconnect).toHaveBeenCalledWith('closed');
    expect(context.onError).toHaveBeenCalledWith(expect.any(Error));
    expect(context.sendPing).toHaveBeenCalledOnce();

    const frame = { close: vi.fn() } as unknown as VideoFrame;
    const tileInfo = { frameSeq: 1, x: 0, y: 0, width: 10, height: 10 } as any;
    captured.videoDecoder.onDecodedFrame(frame, tileInfo);
    captured.videoDecoder.incrementFrameCount();
    captured.videoDecoder.incrementDroppedFrame();
    expect(surfaceRuntime.handleDecodedFrame).toHaveBeenCalledWith(frame, tileInfo);
    expect(stats.frameCount).toBe(1);
    expect(stats.videoFramesDropped).toBe(1);
  });

  it('gracefully skips input-only control wiring when no input controller is attached', () => {
    const factory = new SessionRuntimeFactory({
      createAudioController: vi.fn(() => ({ stopMicrophone: vi.fn(), destroy: vi.fn() } as any)),
      createCameraController: vi.fn(() => ({ stopCamera: vi.fn(), destroy: vi.fn() } as any)),
      createFileTransferController: vi.fn(() => ({ setEnabled: vi.fn(), handleFrame: vi.fn(), destroy: vi.fn() } as any)),
      createSessionCapabilityRuntime: vi.fn(() => ({ apply: vi.fn() } as any)),
      createSessionControlRuntime: vi.fn((options) => options as any),
      createSessionSendRuntime: vi.fn(() => ({ destroy: vi.fn() } as any)),
      createSessionSurfaceRuntime: vi.fn(() => ({ handleDecodedFrame: vi.fn(), markDisplayDirty: vi.fn(), start: vi.fn(), destroy: vi.fn() } as any)),
      createSessionFrameRouterRuntime: vi.fn(() => ({ handleFrame: vi.fn() } as any)),
      createSessionStreamReaderRuntime: vi.fn(() => ({ readStream: vi.fn() } as any)),
      createSessionTransportRuntime: vi.fn(() => ({ connect: vi.fn(), disconnect: vi.fn() } as any)),
      createSessionVideoDecoderRuntime: vi.fn(() => ({ decodeNal: vi.fn(), destroy: vi.fn() } as any)),
    });

    const controlRuntime = factory.create({
      container: createContainer(),
      tileCompositor: {} as any,
      stats: new SessionStats(),
      options: {
        audioEnabled: true,
        fileTransferEnabled: true,
        hiDpi: false,
        pingIntervalMs: 5000,
      },
      context: {
        isConnected: () => true,
        isViewerRestricted: () => false,
        getInputController: () => null,
        setRemoteSize: vi.fn(),
        onResolutionChange: vi.fn(),
        setSessionFlags: vi.fn(),
        setMicrophoneSupported: vi.fn(),
        setCameraSupported: vi.fn(),
        updateCapabilities: vi.fn(),
        applyClientAccessState: vi.fn(),
        handleVideoFrame: vi.fn(),
        handleCursorUpdate: vi.fn(),
        handleClipboardUpdate: vi.fn(),
        handleControlMessage: vi.fn(),
        clearVideoOverlay: vi.fn(),
        onConnect: vi.fn(),
        onDisconnect: vi.fn(),
        onError: vi.fn(),
        handleStream: vi.fn().mockResolvedValue(undefined),
        sendPing: vi.fn(),
        sendResizeRequest: vi.fn(),
        sendTileCacheMiss: vi.fn(),
        sendFrame: vi.fn(),
        sendCameraFrame: vi.fn(() => 'queued' as const),
      },
    }).controlRuntime as any;

    expect(() => {
      controlRuntime.configureInputExtendedKeyEvents(true);
      controlRuntime.sendLayoutHint();
    }).not.toThrow();
  });
});
