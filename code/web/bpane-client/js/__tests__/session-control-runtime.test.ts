import { describe, expect, it, vi } from 'vitest';

import { SessionControlRuntime } from '../session-control-runtime.js';

function createRuntime() {
  const setRemoteSize = vi.fn();
  const onResolutionChange = vi.fn();
  const setSessionFlags = vi.fn();
  const setMicrophoneSupported = vi.fn();
  const setCameraSupported = vi.fn();
  const configureInputExtendedKeyEvents = vi.fn();
  const sendLayoutHint = vi.fn();
  const updateCapabilities = vi.fn();
  const applyClientAccessState = vi.fn();
  const sendControlFrame = vi.fn();

  const runtime = new SessionControlRuntime({
    setRemoteSize,
    onResolutionChange,
    setSessionFlags,
    setMicrophoneSupported,
    setCameraSupported,
    configureInputExtendedKeyEvents,
    sendLayoutHint,
    updateCapabilities,
    applyClientAccessState,
    sendControlFrame,
  });

  return {
    runtime,
    setRemoteSize,
    onResolutionChange,
    setSessionFlags,
    setMicrophoneSupported,
    setCameraSupported,
    configureInputExtendedKeyEvents,
    sendLayoutHint,
    updateCapabilities,
    applyClientAccessState,
    sendControlFrame,
  };
}

describe('SessionControlRuntime', () => {
  it('applies ResolutionAck updates', () => {
    const {
      runtime,
      setRemoteSize,
      onResolutionChange,
    } = createRuntime();

    const payload = new Uint8Array([0x02, 0x00, 0x05, 0x00, 0x03]);
    runtime.handle(payload);

    expect(setRemoteSize).toHaveBeenCalledWith(1280, 768);
    expect(onResolutionChange).toHaveBeenCalledWith(1280, 768);
  });

  it('handles SessionReady flags and keyboard-layout support', () => {
    const {
      runtime,
      setSessionFlags,
      setMicrophoneSupported,
      setCameraSupported,
      configureInputExtendedKeyEvents,
      sendLayoutHint,
      updateCapabilities,
    } = createRuntime();

    const payload = new Uint8Array([0x03, 0x01, 0x38]);
    runtime.handle(payload);

    expect(setSessionFlags).toHaveBeenCalledWith(0x38);
    expect(setMicrophoneSupported).toHaveBeenCalledWith(true);
    expect(setCameraSupported).toHaveBeenCalledWith(true);
    expect(configureInputExtendedKeyEvents).toHaveBeenCalledWith(true);
    expect(sendLayoutHint).toHaveBeenCalledOnce();
    expect(updateCapabilities).toHaveBeenCalledOnce();
  });

  it('responds to Ping with Pong echoing the payload body', () => {
    const {
      runtime,
      sendControlFrame,
    } = createRuntime();

    const payload = new Uint8Array(13);
    payload[0] = 0x04;
    payload.set([0x63, 0x00, 0x00, 0x00, 0x39, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 1);

    runtime.handle(payload);

    const expected = new Uint8Array(13);
    expected[0] = 0x05;
    expected.set(payload.slice(1, 13), 1);
    expect(sendControlFrame).toHaveBeenCalledWith(expected);
  });

  it('maps legacy ResolutionLocked to view-only client access state', () => {
    const {
      runtime,
      applyClientAccessState,
    } = createRuntime();

    const payload = new Uint8Array([0x08, 0x00, 0x05, 0xD0, 0x02]);
    runtime.handle(payload);

    expect(applyClientAccessState).toHaveBeenCalledWith(0x03, 1280, 720);
  });

  it('forwards ClientAccessState with independent flags', () => {
    const {
      runtime,
      applyClientAccessState,
    } = createRuntime();

    const payload = new Uint8Array([0x09, 0x02, 0x00, 0x05, 0xD0, 0x02]);
    runtime.handle(payload);

    expect(applyClientAccessState).toHaveBeenCalledWith(0x02, 1280, 720);
  });

  it('ignores short or unknown control messages', () => {
    const {
      runtime,
      setRemoteSize,
      setSessionFlags,
      applyClientAccessState,
      sendControlFrame,
    } = createRuntime();

    runtime.handle(new Uint8Array());
    runtime.handle(new Uint8Array([0x02]));
    runtime.handle(new Uint8Array([0xff, 0x00, 0x00]));

    expect(setRemoteSize).not.toHaveBeenCalled();
    expect(setSessionFlags).not.toHaveBeenCalled();
    expect(applyClientAccessState).not.toHaveBeenCalled();
    expect(sendControlFrame).not.toHaveBeenCalled();
  });
});
