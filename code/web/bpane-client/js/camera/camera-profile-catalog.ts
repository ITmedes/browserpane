export interface CameraProfile {
  name: string;
  width: number;
  height: number;
  fps: number;
  bitrate: number;
  smooth: boolean | null;
  powerEfficient: boolean | null;
  codec: string;
  keyframeInterval: number;
}

type CameraEncodingInfo = {
  supported: boolean;
  smooth?: boolean;
  powerEfficient?: boolean;
};

type NavigatorWithMediaCapabilities = Navigator & {
  mediaCapabilities?: {
    encodingInfo?: (configuration: unknown) => Promise<CameraEncodingInfo>;
  };
};

export class CameraProfileCatalog {
  private static supportCache: Promise<CameraProfile[]> | null = null;

  private static readonly webRtcContentType =
    'video/H264;level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f';

  private static readonly profiles: CameraProfile[] = [
    {
      name: 'hd720p',
      width: 1280,
      height: 720,
      fps: 30,
      bitrate: 1_600_000,
      keyframeInterval: 30,
      codec: 'avc1.42001f',
      smooth: null,
      powerEfficient: null,
    },
    {
      name: 'qhd540p',
      width: 960,
      height: 540,
      fps: 24,
      bitrate: 950_000,
      keyframeInterval: 24,
      codec: 'avc1.42001f',
      smooth: null,
      powerEfficient: null,
    },
    {
      name: 'nhd360p',
      width: 640,
      height: 360,
      fps: 18,
      bitrate: 450_000,
      keyframeInterval: 18,
      codec: 'avc1.42001e',
      smooth: null,
      powerEfficient: null,
    },
  ];

  static async getSupportedProfiles(): Promise<CameraProfile[]> {
    if (!CameraProfileCatalog.supportCache) {
      CameraProfileCatalog.supportCache = CameraProfileCatalog.probeSupportedProfiles();
    }

    const supportedProfiles = await CameraProfileCatalog.supportCache;
    return supportedProfiles.map((profile) => ({ ...profile }));
  }

  static async probeSupportedProfiles(): Promise<CameraProfile[]> {
    if (
      typeof navigator === 'undefined'
      || !navigator.mediaDevices?.getUserMedia
      || typeof VideoEncoder === 'undefined'
      || typeof VideoFrame === 'undefined'
    ) {
      return [];
    }

    const supportedProfiles: CameraProfile[] = [];
    for (const profile of CameraProfileCatalog.profiles) {
      const encoderConfig = CameraProfileCatalog.toEncoderConfig(profile);
      try {
        const encoderSupport = await VideoEncoder.isConfigSupported(encoderConfig);
        if (!encoderSupport.supported) {
          continue;
        }

        const runtimeProfile = { ...profile };
        const mediaCapabilities = (navigator as NavigatorWithMediaCapabilities).mediaCapabilities;
        if (mediaCapabilities?.encodingInfo) {
          try {
            const info = await mediaCapabilities.encodingInfo({
              type: 'webrtc',
              video: {
                contentType: CameraProfileCatalog.webRtcContentType,
                width: profile.width,
                height: profile.height,
                bitrate: profile.bitrate,
                framerate: profile.fps,
              },
            });
            if (!info.supported) {
              continue;
            }

            runtimeProfile.smooth = typeof info.smooth === 'boolean' ? info.smooth : null;
            runtimeProfile.powerEfficient = typeof info.powerEfficient === 'boolean'
              ? info.powerEfficient
              : null;
          } catch {
            // Ignore media-capabilities probe failures and rely on VideoEncoder support.
          }
        }

        supportedProfiles.push(runtimeProfile);
      } catch {
        // Ignore this rung and try the next one down.
      }
    }

    return CameraProfileCatalog.rankSupportedProfiles(supportedProfiles);
  }

  static getCaptureConstraints(): MediaStreamConstraints {
    return {
      video: {
        width: { ideal: 1280 },
        height: { ideal: 720 },
        frameRate: { ideal: 30, max: 35 },
      },
      audio: false,
    };
  }

  static toEncoderConfig(profile: CameraProfile): VideoEncoderConfig {
    return {
      codec: profile.codec,
      width: profile.width,
      height: profile.height,
      displayWidth: profile.width,
      displayHeight: profile.height,
      bitrate: profile.bitrate,
      framerate: profile.fps,
      latencyMode: 'realtime',
      avc: {
        format: 'annexb',
      },
    };
  }

  static clearSupportCacheForTest(): void {
    CameraProfileCatalog.supportCache = null;
  }

  private static rankSupportedProfiles(supportedProfiles: CameraProfile[]): CameraProfile[] {
    const smoothProfiles = supportedProfiles.filter((profile) => profile.smooth !== false);
    const unsmoothProfiles = supportedProfiles.filter((profile) => profile.smooth === false);
    const rankedProfiles = smoothProfiles.length > 0
      ? smoothProfiles.concat(unsmoothProfiles)
      : supportedProfiles;

    return rankedProfiles.map((profile) => ({ ...profile }));
  }
}
