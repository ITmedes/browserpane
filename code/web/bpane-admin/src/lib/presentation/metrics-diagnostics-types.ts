import type {
  BrowserSessionRenderDiagnostics,
  BrowserSessionStatsSnapshot,
  BrowserSessionTileCacheStats,
} from '../session/browser-session-types';

export type MetricsRawSample = {
  readonly capturedAtMs: number;
  readonly frameCount: number;
  readonly stats: BrowserSessionStatsSnapshot;
  readonly diagnostics: BrowserSessionRenderDiagnostics | null;
  readonly tileCache: BrowserSessionTileCacheStats | null;
};

export type MetricsDiagnosticsPayload = {
  readonly schema: 'browserpane.admin.metrics.sample.v1';
  readonly timing: MetricsTimingSummary;
  readonly frames: MetricsFrameSummary;
  readonly transfer: MetricsTransferSummary;
  readonly tiles: MetricsTileSummary;
  readonly scroll: MetricsScrollDiagnosticsSummary;
  readonly video: MetricsVideoSummary;
  readonly render: BrowserSessionRenderDiagnostics | null;
};

export type ChannelDelta = { readonly bytes: number; readonly frames: number };
export type ChannelSnapshot = { readonly bytes?: number; readonly frames?: number };
export type ScrollFallbackReasons = {
  readonly nonQuantized: number;
  readonly fullRepaint: number;
  readonly lowSavedRatio: number;
  readonly largeRowShift: number;
  readonly other: number;
  readonly zeroSaved: number;
};

type MetricsTimingSummary = {
  readonly startCapturedAtMs: number;
  readonly endCapturedAtMs: number;
  readonly durationMs: number;
  readonly startElapsedMs: number | null;
  readonly endElapsedMs: number | null;
};

type MetricsFrameSummary = { readonly start: number; readonly end: number; readonly delta: number; readonly fps: number };

type MetricsTransferSummary = {
  readonly rxBytes: number;
  readonly txBytes: number;
  readonly rxFrames: number;
  readonly txFrames: number;
  readonly rxByChannel: Readonly<Record<string, ChannelDelta>>;
  readonly txByChannel: Readonly<Record<string, ChannelDelta>>;
  readonly tileBytes: number;
  readonly videoBytes: number;
  readonly avgRxRate: number;
  readonly avgTxRate: number;
  readonly avgTileRate: number;
  readonly avgVideoRate: number;
  readonly peakRxRate: number;
  readonly peakTxRate: number;
  readonly peakTileRate: number;
  readonly peakVideoRate: number;
};

type MetricsTileSummary = {
  readonly commandBytes: number;
  readonly totalCommands: number;
  readonly imageCommands: number;
  readonly videoCommands: number;
  readonly drawCommands: number;
  readonly cache: { readonly hits: number; readonly misses: number; readonly hitRate: number; readonly size: number; readonly bytes: number; readonly evictions: number };
  readonly redundant: { readonly commands: number; readonly bytes: number };
  readonly commands: Readonly<Record<string, number>>;
  readonly batches: { readonly queued: number; readonly totalCommands: number; readonly averageCommands: number; readonly maxCommands: number; readonly maxPendingCommands: number };
};

type MetricsScrollDiagnosticsSummary = {
  readonly batches: number; readonly subTileBatches: number; readonly updateCommands: number; readonly qoiCommands: number;
  readonly cacheHitCommands: number; readonly fillCommands: number; readonly qoiBytes: number;
  readonly savedTiles: number; readonly potentialTiles: number; readonly reuseRate: number;
  readonly subTileSavedTiles: number; readonly subTilePotentialTiles: number; readonly subTileReuseRate: number;
  readonly hostBatches: number; readonly hostFallbacks: number; readonly hostSavedRate: number;
  readonly hostFallbackRate: number; readonly hostFallbackRateRecent20: number; readonly hostFallbackRateRecent50: number;
  readonly fallbackReasons: ScrollFallbackReasons; readonly dominantHostFallbackReason: string;
  readonly hostSplitRegionBatches: number; readonly hostStickyBandBatches: number; readonly hostChromeTiles: number;
  readonly hostSentHashEntries: number; readonly hostSentHashEvictions: number; readonly hostCacheMissReports: number;
};

type MetricsVideoSummary = {
  readonly decodedFrames: number;
  readonly datagrams: number;
  readonly datagramBytes: number;
  readonly droppedFrames: number;
};
