import type {
  BrowserSessionRenderDiagnostics,
  BrowserSessionStatsSnapshot,
} from '../session/browser-session-types';

export type MetricsRawSample = {
  readonly capturedAtMs: number;
  readonly frameCount: number;
  readonly stats: BrowserSessionStatsSnapshot;
  readonly diagnostics: BrowserSessionRenderDiagnostics | null;
};

export type MetricsDiagnosticsPayload = {
  readonly schema: 'browserpane.admin.metrics.sample.v1';
  readonly timing: {
    readonly startCapturedAtMs: number;
    readonly endCapturedAtMs: number;
    readonly durationMs: number;
    readonly startElapsedMs: number | null;
    readonly endElapsedMs: number | null;
  };
  readonly frames: {
    readonly start: number;
    readonly end: number;
    readonly delta: number;
    readonly fps: number;
  };
  readonly transfer: {
    readonly rxBytes: number;
    readonly txBytes: number;
    readonly rxFrames: number;
    readonly txFrames: number;
    readonly rxByChannel: Readonly<Record<string, ChannelDelta>>;
    readonly txByChannel: Readonly<Record<string, ChannelDelta>>;
  };
  readonly tiles: {
    readonly commandBytes: number;
    readonly totalCommands: number;
    readonly imageCommands: number;
    readonly videoCommands: number;
    readonly drawCommands: number;
    readonly cache: TileCacheSummary;
    readonly redundant: TileRedundantSummary;
    readonly commands: Readonly<Record<string, number>>;
  };
  readonly scroll: ScrollDiagnosticsSummary;
  readonly video: {
    readonly decodedFrames: number;
    readonly datagrams: number;
    readonly datagramBytes: number;
    readonly droppedFrames: number;
  };
  readonly render: BrowserSessionRenderDiagnostics | null;
};

type ChannelDelta = { readonly bytes: number; readonly frames: number };
type ChannelSnapshot = { readonly bytes?: number; readonly frames?: number };
type TileCacheSummary = { readonly hits: number; readonly misses: number; readonly hitRate: number; readonly size: number };
type TileRedundantSummary = { readonly commands: number; readonly bytes: number };

type ScrollDiagnosticsSummary = {
  readonly batches: number;
  readonly subTileBatches: number;
  readonly updateCommands: number;
  readonly qoiCommands: number;
  readonly cacheHitCommands: number;
  readonly fillCommands: number;
  readonly qoiBytes: number;
  readonly savedTiles: number;
  readonly potentialTiles: number;
  readonly reuseRate: number;
  readonly subTileSavedTiles: number;
  readonly subTilePotentialTiles: number;
  readonly subTileReuseRate: number;
  readonly hostFallbackRate: number;
  readonly hostFallbackRateRecent20: number;
  readonly hostFallbackRateRecent50: number;
};

export class MetricsDiagnosticsPayloadBuilder {
  static build(start: MetricsRawSample, end: MetricsRawSample): MetricsDiagnosticsPayload {
    const durationMs = Math.max(0, end.capturedAtMs - start.capturedAtMs);
    const durationSec = Math.max(durationMs / 1000, 0.001);
    const frames = delta(end.frameCount, start.frameCount);
    return {
      schema: 'browserpane.admin.metrics.sample.v1',
      timing: {
        startCapturedAtMs: start.capturedAtMs,
        endCapturedAtMs: end.capturedAtMs,
        durationMs,
        startElapsedMs: nullableNumber(start.stats.elapsedMs),
        endElapsedMs: nullableNumber(end.stats.elapsedMs),
      },
      frames: { start: start.frameCount, end: end.frameCount, delta: frames, fps: frames / durationSec },
      transfer: transferSummary(start.stats, end.stats),
      tiles: tileSummary(start.stats, end.stats),
      scroll: scrollSummary(start.stats, end.stats),
      video: videoSummary(start.stats, end.stats),
      render: end.diagnostics,
    };
  }
}

function transferSummary(start: BrowserSessionStatsSnapshot, end: BrowserSessionStatsSnapshot) {
  return {
    rxBytes: delta(end.transfer?.rxBytes, start.transfer?.rxBytes),
    txBytes: delta(end.transfer?.txBytes, start.transfer?.txBytes),
    rxFrames: delta(end.transfer?.rxFrames, start.transfer?.rxFrames),
    txFrames: delta(end.transfer?.txFrames, start.transfer?.txFrames),
    rxByChannel: channelDeltas(start.transfer?.rxByChannel, end.transfer?.rxByChannel),
    txByChannel: channelDeltas(start.transfer?.txByChannel, end.transfer?.txByChannel),
  };
}

function tileSummary(start: BrowserSessionStatsSnapshot, end: BrowserSessionStatsSnapshot) {
  return {
    commandBytes: delta(end.tiles?.commandBytes, start.tiles?.commandBytes),
    totalCommands: delta(end.tiles?.totalCommands, start.tiles?.totalCommands),
    imageCommands: delta(end.tiles?.imageCommands, start.tiles?.imageCommands),
    videoCommands: delta(end.tiles?.videoCommands, start.tiles?.videoCommands),
    drawCommands: delta(end.tiles?.drawCommands, start.tiles?.drawCommands),
    cache: {
      hits: numberValue(end.tiles?.cacheHitsObserved),
      misses: numberValue(end.tiles?.cacheMissesObserved),
      hitRate: numberValue(end.tiles?.cacheHitRateObserved),
      size: numberValue(end.tiles?.cacheSizeObserved),
    },
    redundant: {
      commands: delta(end.tiles?.redundantQoiCommands, start.tiles?.redundantQoiCommands),
      bytes: delta(end.tiles?.redundantQoiBytes, start.tiles?.redundantQoiBytes),
    },
    commands: numericRecordDeltas(start.tiles?.commands, end.tiles?.commands),
  };
}

function scrollSummary(start: BrowserSessionStatsSnapshot, end: BrowserSessionStatsSnapshot) {
  const startScroll = start.tiles?.scrollComposition;
  const endScroll = end.tiles?.scrollComposition;
  const health = end.tiles?.scrollHealth;
  return {
    batches: delta(endScroll?.scrollBatches, startScroll?.scrollBatches),
    subTileBatches: delta(endScroll?.subTileScrollBatches, startScroll?.subTileScrollBatches),
    updateCommands: delta(endScroll?.scrollUpdateCommands, startScroll?.scrollUpdateCommands),
    qoiCommands: delta(endScroll?.scrollQoiCommands, startScroll?.scrollQoiCommands),
    cacheHitCommands: delta(endScroll?.scrollCacheHitCommands, startScroll?.scrollCacheHitCommands),
    fillCommands: delta(endScroll?.scrollFillCommands, startScroll?.scrollFillCommands),
    qoiBytes: delta(endScroll?.scrollQoiBytes, startScroll?.scrollQoiBytes),
    savedTiles: delta(endScroll?.scrollSavedTiles, startScroll?.scrollSavedTiles),
    potentialTiles: delta(endScroll?.scrollPotentialTiles, startScroll?.scrollPotentialTiles),
    reuseRate: numberValue(endScroll?.scrollReuseRate),
    subTileSavedTiles: delta(endScroll?.subTileScrollSavedTiles, startScroll?.subTileScrollSavedTiles),
    subTilePotentialTiles: delta(endScroll?.subTileScrollPotentialTiles, startScroll?.subTileScrollPotentialTiles),
    subTileReuseRate: numberValue(endScroll?.subTileScrollReuseRate),
    hostFallbackRate: numberValue(health?.hostFallbackRate),
    hostFallbackRateRecent20: numberValue(health?.hostFallbackRateRecent20),
    hostFallbackRateRecent50: numberValue(health?.hostFallbackRateRecent50),
  };
}

function videoSummary(start: BrowserSessionStatsSnapshot, end: BrowserSessionStatsSnapshot) {
  return {
    decodedFrames: delta(end.video?.decodedFrames, start.video?.decodedFrames),
    datagrams: delta(end.video?.datagrams, start.video?.datagrams),
    datagramBytes: delta(end.video?.datagramBytes, start.video?.datagramBytes),
    droppedFrames: delta(end.video?.droppedFrames, start.video?.droppedFrames),
  };
}

function channelDeltas(
  start: Readonly<Record<string, ChannelSnapshot>> | undefined,
  end: Readonly<Record<string, ChannelSnapshot>> | undefined,
): Readonly<Record<string, ChannelDelta>> {
  const keys = new Set([...Object.keys(start ?? {}), ...Object.keys(end ?? {})]);
  return Object.fromEntries([...keys].sort().map((key) => [
    key,
    { bytes: delta(end?.[key]?.bytes, start?.[key]?.bytes), frames: delta(end?.[key]?.frames, start?.[key]?.frames) },
  ]));
}

function numericRecordDeltas(
  start: Readonly<Record<string, number>> | undefined,
  end: Readonly<Record<string, number>> | undefined,
): Readonly<Record<string, number>> {
  const keys = new Set([...Object.keys(start ?? {}), ...Object.keys(end ?? {})]);
  return Object.fromEntries([...keys].sort().map((key) => [key, delta(end?.[key], start?.[key])]));
}

function nullableNumber(value: number | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function numberValue(value: number | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

function delta(end: number | undefined, start: number | undefined): number {
  return Math.max(0, numberValue(end) - numberValue(start));
}
