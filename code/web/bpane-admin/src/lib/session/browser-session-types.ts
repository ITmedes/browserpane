export type BrowserSessionRenderBackend = 'auto' | 'canvas2d' | 'webgl2';

export type BrowserSessionRecordingOptions = {
  readonly frameRate?: number;
  readonly mimeType?: string;
  readonly videoBitsPerSecond?: number;
  readonly audioBitsPerSecond?: number;
};

export type BrowserSessionRenderDiagnostics = {
  readonly backend: string;
  readonly reason: string;
  readonly renderer?: string | null;
  readonly vendor?: string | null;
  readonly software?: boolean;
};

export type BrowserSessionStatsSnapshot = {
  readonly elapsedMs?: number;
  readonly transfer?: {
    readonly rxBytes?: number;
    readonly txBytes?: number;
    readonly rxFrames?: number;
    readonly txFrames?: number;
    readonly rxByChannel?: Readonly<Record<string, { readonly bytes?: number; readonly frames?: number }>>;
    readonly txByChannel?: Readonly<Record<string, { readonly bytes?: number; readonly frames?: number }>>;
  };
  readonly tiles?: {
    readonly commandBytes?: number;
    readonly commands?: Readonly<Record<string, number>>;
    readonly imageCommands?: number;
    readonly videoCommands?: number;
    readonly drawCommands?: number;
    readonly totalCommands?: number;
    readonly cacheHitsObserved?: number;
    readonly cacheMissesObserved?: number;
    readonly cacheHitRateObserved?: number;
    readonly cacheSizeObserved?: number;
    readonly redundantQoiCommands?: number;
    readonly redundantQoiBytes?: number;
    readonly scrollComposition?: {
      readonly scrollBatches?: number;
      readonly subTileScrollBatches?: number;
      readonly scrollUpdateCommands?: number;
      readonly scrollQoiCommands?: number;
      readonly scrollCacheHitCommands?: number;
      readonly scrollFillCommands?: number;
      readonly scrollQoiBytes?: number;
      readonly scrollSavedTiles?: number;
      readonly scrollPotentialTiles?: number;
      readonly scrollReuseRate?: number;
      readonly subTileScrollSavedTiles?: number;
      readonly subTileScrollPotentialTiles?: number;
      readonly subTileScrollReuseRate?: number;
    };
    readonly scrollHealth?: {
      readonly hostFallbackRate?: number;
      readonly hostFallbackRateRecent20?: number;
      readonly hostFallbackRateRecent50?: number;
    };
  };
  readonly video?: {
    readonly decodedFrames?: number;
    readonly datagrams?: number;
    readonly droppedFrames?: number;
    readonly datagramBytes?: number;
  };
};

export type BrowserSessionConnectPreferences = {
  readonly hiDpi: boolean;
  readonly audio: boolean;
  readonly microphone: boolean;
  readonly camera: boolean;
  readonly clipboard: boolean;
  readonly fileTransfer: boolean;
  readonly renderBackend: BrowserSessionRenderBackend;
  readonly scrollCopy: boolean;
};

export const DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES = Object.freeze({
  hiDpi: true,
  audio: true,
  microphone: true,
  camera: true,
  clipboard: true,
  fileTransfer: true,
  renderBackend: 'auto',
  scrollCopy: true,
} satisfies BrowserSessionConnectPreferences);

export type BrowserSessionConnectOptions = {
  readonly container: HTMLElement;
  readonly gatewayUrl: string;
  readonly connectTicket?: string;
  readonly accessToken?: string;
  readonly clientRole?: 'interactive' | 'recorder';
  readonly hiDpi?: boolean;
  readonly audio?: boolean;
  readonly microphone?: boolean;
  readonly camera?: boolean;
  readonly clipboard?: boolean;
  readonly fileTransfer?: boolean;
  readonly certHashUrl?: string;
  readonly renderBackend?: BrowserSessionRenderBackend;
  readonly scrollCopy?: boolean;
  readonly onConnect?: () => void;
  readonly onDisconnect?: (reason: string) => void;
  readonly onError?: (error: Error) => void;
};

export type BrowserSessionHandle = {
  readonly disconnect: () => void;
  readonly getFrameCount?: () => number;
  readonly startMicrophone?: () => Promise<void>;
  readonly stopMicrophone?: () => void;
  readonly startCamera?: () => Promise<void>;
  readonly stopCamera?: () => void;
  readonly uploadFiles?: (files: FileList | Iterable<File>) => Promise<void>;
  readonly isRecording?: () => boolean;
  readonly startRecording?: (options?: BrowserSessionRecordingOptions) => Promise<void>;
  readonly stopRecording?: () => Promise<Blob>;
  readonly getSessionStats?: () => BrowserSessionStatsSnapshot;
  readonly getRenderDiagnostics?: () => BrowserSessionRenderDiagnostics;
};

export type BrowserSessionSdk = {
  readonly BpaneSession: {
    readonly connect: (options: BrowserSessionConnectOptions) => Promise<BrowserSessionHandle>;
  };
};

export type LiveBrowserSessionConnection = {
  readonly sessionId: string;
  readonly gatewayUrl: string;
  readonly handle: BrowserSessionHandle;
};
