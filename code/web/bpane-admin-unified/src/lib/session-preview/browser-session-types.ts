export type BrowserSessionRenderBackend = 'auto' | 'canvas2d' | 'webgl2';
export type BrowserSessionResizeSource = 'canvas' | 'container';

export type BrowserSessionConnectPreferences = {
  readonly hiDpi: boolean;
  readonly audio: boolean;
  readonly microphone: boolean;
  readonly camera: boolean;
  readonly clipboard: boolean;
  readonly fileTransfer: boolean;
  readonly renderBackend: BrowserSessionRenderBackend;
  readonly resizeSource: BrowserSessionResizeSource;
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
  resizeSource: 'container',
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
  readonly resizeSource?: BrowserSessionResizeSource;
  readonly scrollCopy?: boolean;
  readonly onConnect?: () => void;
  readonly onDisconnect?: (reason: string) => void;
  readonly onError?: (error: Error) => void;
};

export type BrowserSessionRenderDiagnostics = {
  readonly backend: 'webgl2' | 'canvas2d' | string;
  readonly reason: string;
  readonly renderer?: string | null;
  readonly vendor?: string | null;
  readonly software?: boolean;
};

export type BrowserSessionTileRuntimeStats = {
  readonly hits: number;
  readonly misses: number;
  readonly hitRate: number;
  readonly size: number;
  readonly bytes: number;
  readonly evictions: number;
  readonly fills: number;
  readonly qoiDecodes: number;
  readonly qoiRedundant: number;
  readonly qoiRedundantBytes: number;
  readonly zstdDecodes: number;
  readonly zstdRedundant: number;
  readonly zstdRedundantBytes: number;
  readonly cacheMisses: number;
  readonly scrollCopies: number;
  readonly batchesQueued: number;
  readonly totalBatchCommands: number;
  readonly maxBatchCommands: number;
  readonly lastBatchCommands: number;
  readonly currentPendingCommands: number;
  readonly pendingCommandsHighWaterMark: number;
};

export type BrowserSessionChannelTransferStats = {
  readonly bytes: number;
  readonly frames: number;
};

export type BrowserSessionStatsSnapshot = {
  readonly elapsedMs: number;
  readonly transfer: {
    readonly rxBytes: number;
    readonly txBytes: number;
    readonly rxFrames: number;
    readonly txFrames: number;
    readonly rxByChannel: Readonly<Record<string, BrowserSessionChannelTransferStats>>;
    readonly txByChannel: Readonly<Record<string, BrowserSessionChannelTransferStats>>;
  };
  readonly tiles: {
    readonly commandBytes: number;
    readonly imageCommands: number;
    readonly videoCommands: number;
    readonly drawCommands: number;
    readonly totalCommands: number;
    readonly cacheHitsObserved: number;
    readonly cacheMissesObserved: number;
    readonly cacheHitRateObserved: number;
    readonly cacheSizeObserved: number;
    readonly redundantQoiCommands: number;
    readonly redundantQoiBytes: number;
    readonly scrollComposition: {
      readonly scrollBatches: number;
      readonly subTileScrollBatches: number;
      readonly scrollPotentialTiles: number;
      readonly scrollSavedTiles: number;
      readonly subTileScrollPotentialTiles: number;
      readonly subTileScrollSavedTiles: number;
      readonly subTileScrollReuseRate: number;
      readonly subTileComposeCandidateRate: number;
    };
    readonly scrollHealth: {
      readonly hostScrollBatchesTotal: number;
      readonly hostScrollFallbacksTotal: number;
      readonly hostScrollNonQuantizedFallbacksTotal: number;
      readonly hostScrollResidualFullRepaintsTotal: number;
      readonly hostScrollResidualInteriorLimitFallbacksTotal: number;
      readonly hostScrollResidualLowSavedRatioFallbacksTotal: number;
      readonly hostScrollResidualLargeRowShiftFallbacksTotal: number;
      readonly hostScrollResidualOtherFallbacksTotal: number;
      readonly hostScrollZeroSavedBatchesTotal: number;
      readonly hostScrollSplitRegionBatchesTotal: number;
      readonly hostScrollStickyBandBatchesTotal: number;
      readonly hostScrollChromeTilesTotal: number;
      readonly hostScrollExposedStripTilesTotal: number;
      readonly hostScrollInteriorResidualTilesTotal: number;
      readonly hostScrollEdgeStripResidualTilesTotal: number;
      readonly hostScrollSmallEdgeStripResidualTilesTotal: number;
      readonly hostScrollSmallEdgeStripResidualRowsTotal: number;
      readonly hostScrollSmallEdgeStripResidualAreaPxTotal: number;
      readonly hostSentHashEntries: number;
      readonly hostSentHashEvictionsTotal: number;
      readonly hostCacheMissReportsTotal: number;
      readonly hostFallbackRate: number;
      readonly hostFallbackRateRecent20: number;
      readonly hostFallbackRateRecent50: number;
      readonly hostFallbackRateRecent20Batches: number;
      readonly hostFallbackRateRecent50Batches: number;
      readonly hostScrollPotentialTilesTotal: number;
      readonly hostScrollSavedTilesTotal: number;
      readonly hostScrollSavedRate: number;
      readonly lastHostScrollStatsAtMs: number;
    };
  };
  readonly video: {
    readonly decodedFrames: number;
    readonly droppedFrames: number;
    readonly datagrams: number;
    readonly datagramBytes: number;
  };
};

export type BrowserSessionHandle = {
  readonly disconnect: () => void;
  readonly getFrameCount?: () => number;
  readonly getTileCacheStats?: () => BrowserSessionTileRuntimeStats;
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
