import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CameraProfileCatalog } from '../camera/camera-profile-catalog.js';

type MutableNavigator = {
  mediaDevices?: MediaDevices;
  mediaCapabilities?: {
    encodingInfo?: (configuration: unknown) => Promise<{
      supported: boolean;
      smooth?: boolean;
      powerEfficient?: boolean;
    }>;
  };
};

class MockVideoEncoder {
  static isConfigSupported = vi.fn(async (config: VideoEncoderConfig) => ({
    supported: config.width !== 960,
  }));
}

class MockVideoFrame {}

describe('CameraProfileCatalog', () => {
  beforeEach(() => {
    const mutableNavigator = globalThis.navigator as unknown as MutableNavigator;

    mutableNavigator.mediaDevices = {
      getUserMedia: vi.fn(),
    } as unknown as MediaDevices;

    mutableNavigator.mediaCapabilities = {
      encodingInfo: vi.fn(async (configuration: unknown) => {
        const video = (configuration as {
          video?: {
            width?: number;
            height?: number;
          };
        }).video;
        return {
          supported: true,
          smooth: (video?.width ?? 0) <= 960,
          powerEfficient: (video?.height ?? 0) <= 540,
        };
      }),
    };

    vi.stubGlobal('VideoEncoder', MockVideoEncoder);
    vi.stubGlobal('VideoFrame', MockVideoFrame);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('returns no supported profiles when required browser APIs are unavailable', async () => {
    const mutableNavigator = globalThis.navigator as unknown as MutableNavigator;

    mutableNavigator.mediaDevices = undefined;
    vi.stubGlobal('VideoEncoder', undefined);
    vi.stubGlobal('VideoFrame', undefined);

    await expect(CameraProfileCatalog.probeSupportedProfiles()).resolves.toEqual([]);
  });

  it('skips unsupported rungs and prefers smooth profiles first', async () => {
    await expect(CameraProfileCatalog.probeSupportedProfiles()).resolves.toMatchObject([
      {
        name: 'nhd360p',
        width: 640,
        height: 360,
        smooth: true,
        powerEfficient: true,
      },
      {
        name: 'hd720p',
        width: 1280,
        height: 720,
        smooth: false,
        powerEfficient: false,
      },
    ]);
  });

  it('ignores media capability probe failures and falls back to encoder support', async () => {
    const mutableNavigator = globalThis.navigator as unknown as MutableNavigator;

    mutableNavigator.mediaCapabilities = {
      encodingInfo: vi.fn(async () => {
        throw new Error('probe failed');
      }),
    };

    await expect(CameraProfileCatalog.probeSupportedProfiles()).resolves.toMatchObject([
      {
        name: 'hd720p',
        width: 1280,
        height: 720,
        smooth: null,
        powerEfficient: null,
      },
      {
        name: 'nhd360p',
        width: 640,
        height: 360,
        smooth: null,
        powerEfficient: null,
      },
    ]);
  });

  it('builds a realtime annex-b encoder config for each selected profile', () => {
    expect(CameraProfileCatalog.toEncoderConfig({
      name: 'hd720p',
      width: 1280,
      height: 720,
      fps: 30,
      bitrate: 1_600_000,
      keyframeInterval: 30,
      codec: 'avc1.42001f',
      smooth: null,
      powerEfficient: null,
    })).toEqual({
      codec: 'avc1.42001f',
      width: 1280,
      height: 720,
      displayWidth: 1280,
      displayHeight: 720,
      bitrate: 1_600_000,
      framerate: 30,
      latencyMode: 'realtime',
      avc: {
        format: 'annexb',
      },
    });
  });
});
