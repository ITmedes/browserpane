import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CameraSessionResources } from '../camera/camera-session-resources.js';

describe('CameraSessionResources', () => {
  const trackStop = vi.fn();

  beforeEach(() => {
    trackStop.mockReset();

    (globalThis.navigator as any).mediaDevices = {
      getUserMedia: vi.fn(async () => ({
        getTracks: () => [{ stop: trackStop }],
      }) as unknown as MediaStream),
    };

    vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined);
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
      drawImage: vi.fn(),
    } as unknown as CanvasRenderingContext2D);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('opens camera session resources with stream, video element, and canvas context', async () => {
    const resources = new CameraSessionResources();

    const snapshot = await resources.open();

    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith(expect.objectContaining({
      video: expect.objectContaining({
        width: { ideal: 1280 },
        height: { ideal: 720 },
        frameRate: expect.objectContaining({ ideal: 30 }),
      }),
      audio: false,
    }));
    expect(snapshot.videoElement.muted).toBe(true);
    expect(snapshot.videoElement.playsInline).toBe(true);
    expect(snapshot.videoElement.autoplay).toBe(true);
    expect(snapshot.videoElement.srcObject).toBe(snapshot.stream);
    expect(snapshot.canvasContext).toBeTruthy();
  });

  it('closes the session and releases the stream and media elements', async () => {
    const resources = new CameraSessionResources();
    const snapshot = await resources.open();

    resources.close();

    expect(trackStop).toHaveBeenCalledTimes(1);
    expect(snapshot.videoElement.pause).toHaveBeenCalledTimes(1);
    expect(snapshot.videoElement.srcObject).toBeNull();
    expect(resources.getSnapshot()).toBeNull();
  });

  it('cleans up when the canvas context is unavailable', async () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
    const resources = new CameraSessionResources();

    await expect(resources.open()).rejects.toThrow('camera canvas context unavailable');

    expect(trackStop).toHaveBeenCalledTimes(1);
    expect(resources.getSnapshot()).toBeNull();
  });

  it('cleans up when video playback startup fails', async () => {
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockRejectedValue(new Error('play failed'));
    const resources = new CameraSessionResources();

    await expect(resources.open()).rejects.toThrow('play failed');

    expect(trackStop).toHaveBeenCalledTimes(1);
    expect(resources.getSnapshot()).toBeNull();
  });
});
