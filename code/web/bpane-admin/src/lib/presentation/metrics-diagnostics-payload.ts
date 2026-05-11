import type { BrowserSessionStatsSnapshot } from '../session/browser-session-types';
import type {
  ChannelDelta,
  ChannelSnapshot,
  MetricsDiagnosticsPayload,
  MetricsRawSample,
  ScrollFallbackReasons,
} from './metrics-diagnostics-types';
import type { MetricsSampleExtrema } from './metrics-sample-extrema';

export type { MetricsDiagnosticsPayload, MetricsRawSample } from './metrics-diagnostics-types';

export class MetricsDiagnosticsPayloadBuilder {
  static build(start: MetricsRawSample, end: MetricsRawSample, extrema?: MetricsSampleExtrema): MetricsDiagnosticsPayload {
    const durationMs = Math.max(0, end.capturedAtMs - start.capturedAtMs);
    const durationSec = Math.max(durationMs / 1000, 0.001);
    const frames = delta(end.frameCount, start.frameCount);
    const transfer = transferSummary(start, end, durationSec, extrema);
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
      transfer,
      tiles: tileSummary(start, end, extrema),
      scroll: scrollSummary(start.stats, end.stats),
      video: videoSummary(start.stats, end.stats),
      render: end.diagnostics,
    };
  }
}

function transferSummary(
  start: MetricsRawSample,
  end: MetricsRawSample,
  durationSec: number,
  extrema: MetricsSampleExtrema | undefined,
) {
  const rxBytes = delta(end.stats.transfer?.rxBytes, start.stats.transfer?.rxBytes);
  const txBytes = delta(end.stats.transfer?.txBytes, start.stats.transfer?.txBytes);
  const tileBytes = delta(end.stats.tiles?.commandBytes, start.stats.tiles?.commandBytes);
  const videoBytes = delta(end.stats.video?.datagramBytes, start.stats.video?.datagramBytes);
  return {
    rxBytes,
    txBytes,
    rxFrames: delta(end.stats.transfer?.rxFrames, start.stats.transfer?.rxFrames),
    txFrames: delta(end.stats.transfer?.txFrames, start.stats.transfer?.txFrames),
    rxByChannel: channelDeltas(start.stats.transfer?.rxByChannel, end.stats.transfer?.rxByChannel),
    txByChannel: channelDeltas(start.stats.transfer?.txByChannel, end.stats.transfer?.txByChannel),
    tileBytes,
    videoBytes,
    avgRxRate: rxBytes / durationSec,
    avgTxRate: txBytes / durationSec,
    avgTileRate: tileBytes / durationSec,
    avgVideoRate: videoBytes / durationSec,
    peakRxRate: extrema?.peakRxRate ?? rxBytes / durationSec,
    peakTxRate: extrema?.peakTxRate ?? txBytes / durationSec,
    peakTileRate: extrema?.peakTileRate ?? tileBytes / durationSec,
    peakVideoRate: extrema?.peakVideoRate ?? videoBytes / durationSec,
  };
}

function tileSummary(start: MetricsRawSample, end: MetricsRawSample, extrema: MetricsSampleExtrema | undefined) {
  const batchesQueued = delta(end.tileCache?.batchesQueued, start.tileCache?.batchesQueued);
  const totalBatchCommands = delta(end.tileCache?.totalBatchCommands, start.tileCache?.totalBatchCommands);
  return {
    commandBytes: delta(end.stats.tiles?.commandBytes, start.stats.tiles?.commandBytes),
    totalCommands: delta(end.stats.tiles?.totalCommands, start.stats.tiles?.totalCommands),
    imageCommands: delta(end.stats.tiles?.imageCommands, start.stats.tiles?.imageCommands),
    videoCommands: delta(end.stats.tiles?.videoCommands, start.stats.tiles?.videoCommands),
    drawCommands: delta(end.stats.tiles?.drawCommands, start.stats.tiles?.drawCommands),
    cache: {
      hits: numberValue(end.stats.tiles?.cacheHitsObserved),
      misses: numberValue(end.stats.tiles?.cacheMissesObserved),
      hitRate: numberValue(end.stats.tiles?.cacheHitRateObserved),
      size: numberValue(end.stats.tiles?.cacheSizeObserved),
      bytes: numberValue(end.tileCache?.bytes),
      evictions: delta(end.tileCache?.evictions, start.tileCache?.evictions),
    },
    redundant: {
      commands: delta(end.stats.tiles?.redundantQoiCommands, start.stats.tiles?.redundantQoiCommands),
      bytes: delta(end.stats.tiles?.redundantQoiBytes, start.stats.tiles?.redundantQoiBytes),
    },
    commands: numericRecordDeltas(start.stats.tiles?.commands, end.stats.tiles?.commands),
    batches: {
      queued: batchesQueued,
      totalCommands: totalBatchCommands,
      averageCommands: batchesQueued > 0 ? totalBatchCommands / batchesQueued : 0,
      maxCommands: extrema?.maxBatchCommands ?? numberValue(end.tileCache?.maxBatchCommands),
      maxPendingCommands: extrema?.maxPendingCommands ?? numberValue(end.tileCache?.pendingCommandsHighWaterMark),
    },
  };
}

function scrollSummary(start: BrowserSessionStatsSnapshot, end: BrowserSessionStatsSnapshot) {
  const startScroll = start.tiles?.scrollComposition;
  const endScroll = end.tiles?.scrollComposition;
  const health = end.tiles?.scrollHealth;
  const startHealth = start.tiles?.scrollHealth;
  const fallbackReasons = {
    nonQuantized: delta(health?.hostScrollNonQuantizedFallbacksTotal, startHealth?.hostScrollNonQuantizedFallbacksTotal),
    fullRepaint: delta(health?.hostScrollResidualFullRepaintsTotal, startHealth?.hostScrollResidualFullRepaintsTotal),
    lowSavedRatio: delta(health?.hostScrollResidualLowSavedRatioFallbacksTotal, startHealth?.hostScrollResidualLowSavedRatioFallbacksTotal),
    largeRowShift: delta(health?.hostScrollResidualLargeRowShiftFallbacksTotal, startHealth?.hostScrollResidualLargeRowShiftFallbacksTotal),
    other: delta(health?.hostScrollResidualOtherFallbacksTotal, startHealth?.hostScrollResidualOtherFallbacksTotal),
    zeroSaved: delta(health?.hostScrollZeroSavedBatchesTotal, startHealth?.hostScrollZeroSavedBatchesTotal),
  };
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
    hostBatches: delta(health?.hostScrollBatchesTotal, startHealth?.hostScrollBatchesTotal),
    hostFallbacks: delta(health?.hostScrollFallbacksTotal, startHealth?.hostScrollFallbacksTotal),
    hostSavedRate: numberValue(health?.hostScrollSavedRate),
    hostFallbackRate: numberValue(health?.hostFallbackRate),
    hostFallbackRateRecent20: numberValue(health?.hostFallbackRateRecent20),
    hostFallbackRateRecent50: numberValue(health?.hostFallbackRateRecent50),
    fallbackReasons,
    dominantHostFallbackReason: dominantReason(fallbackReasons),
    hostSplitRegionBatches: delta(health?.hostScrollSplitRegionBatchesTotal, startHealth?.hostScrollSplitRegionBatchesTotal),
    hostStickyBandBatches: delta(health?.hostScrollStickyBandBatchesTotal, startHealth?.hostScrollStickyBandBatchesTotal),
    hostChromeTiles: delta(health?.hostScrollChromeTilesTotal, startHealth?.hostScrollChromeTilesTotal),
    hostSentHashEntries: numberValue(health?.hostSentHashEntries),
    hostSentHashEvictions: delta(health?.hostSentHashEvictionsTotal, startHealth?.hostSentHashEvictionsTotal),
    hostCacheMissReports: delta(health?.hostCacheMissReportsTotal, startHealth?.hostCacheMissReportsTotal),
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

function dominantReason(reasons: ScrollFallbackReasons): string {
  return Object.entries(reasons).reduce(
    (best, [label, value]) => value > best.value ? { label, value } : best,
    { label: 'none', value: 0 },
  ).label;
}
