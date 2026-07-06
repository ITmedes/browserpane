import type {
  BrowserSessionHandle,
  BrowserSessionRenderDiagnostics,
  BrowserSessionStatsSnapshot,
  BrowserSessionTileRuntimeStats,
} from './browser-session-types';

export type SessionPreviewMetricsStatus = 'idle' | 'running' | 'stopped';

export type SessionPreviewMetricsSummary = {
  readonly status: SessionPreviewMetricsStatus;
  readonly durationMs: number;
  readonly frameCount: number;
  readonly averageFps: number;
  readonly render: {
    readonly backend: string;
    readonly reason: string;
    readonly renderer: string;
    readonly label: string;
  };
  readonly transfer: {
    readonly rxBytes: number;
    readonly txBytes: number;
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
  readonly tiles: {
    readonly totalCommands: number;
    readonly imageCommands: number;
    readonly videoCommands: number;
    readonly drawCommands: number;
    readonly cacheHitRateObserved: number;
    readonly cacheSizeObserved: number;
    readonly cacheBytesObserved: number;
    readonly cacheHits: number;
    readonly cacheMisses: number;
    readonly cacheEvictions: number;
    readonly maxBatchCommands: number;
    readonly maxPendingCommands: number;
  };
  readonly scroll: {
    readonly scrollBatches: number;
    readonly savedRate: number;
    readonly hostScrollBatches: number;
    readonly hostFallbacks: number;
    readonly hostFallbackRate: number;
    readonly hostFallbackRateRecent20: number;
    readonly hostFallbackRateRecent50: number;
    readonly dominantHostFallbackReason: string;
    readonly hostFallbackReasons: {
      readonly nonQuantizedFallbacks: number;
      readonly residualFullRepaints: number;
      readonly residualInteriorLimitFallbacks: number;
      readonly residualLowSavedRatioFallbacks: number;
      readonly residualLargeRowShiftFallbacks: number;
      readonly residualOtherFallbacks: number;
      readonly zeroSavedBatches: number;
    };
  };
  readonly video: {
    readonly datagrams: number;
    readonly droppedFrames: number;
  };
};

export type BrowserSessionMetricsHandle = BrowserSessionHandle & Required<Pick<
  BrowserSessionHandle,
  'getFrameCount' | 'getTileCacheStats' | 'getSessionStats' | 'getRenderDiagnostics'
>>;

type RuntimeSample = {
  readonly capturedAtMs: number;
  readonly frameCount: number;
  readonly tileRuntime: BrowserSessionTileRuntimeStats;
  readonly sessionStats: BrowserSessionStatsSnapshot;
  readonly render: BrowserSessionRenderDiagnostics;
};

type MetricsBaseline = {
  readonly capturedAtMs: number;
  readonly frameCount: number;
  readonly rxBytes: number;
  readonly txBytes: number;
  readonly tileBytes: number;
  readonly videoBytes: number;
  readonly droppedFrames: number;
  readonly datagrams: number;
  readonly imageCommands: number;
  readonly videoCommands: number;
  readonly drawCommands: number;
  readonly totalCommands: number;
  readonly scrollBatches: number;
  readonly scrollPotentialTiles: number;
  readonly scrollSavedTiles: number;
  readonly hostScrollBatches: number;
  readonly hostFallbacks: number;
  readonly hostNonQuantizedFallbacks: number;
  readonly hostResidualFullRepaints: number;
  readonly hostResidualInteriorLimitFallbacks: number;
  readonly hostResidualLowSavedRatioFallbacks: number;
  readonly hostResidualLargeRowShiftFallbacks: number;
  readonly hostResidualOtherFallbacks: number;
  readonly hostZeroSavedBatches: number;
  readonly cacheHits: number;
  readonly cacheMisses: number;
  readonly cacheEvictions: number;
};

export class SessionPreviewMetricsSampler {
  readonly #handle: BrowserSessionHandle;
  #baseline: MetricsBaseline | null = null;
  #lastSample: RuntimeSample | null = null;
  #summary: SessionPreviewMetricsSummary | null = null;
  #active = false;
  #peakRxRate = 0;
  #peakTxRate = 0;
  #peakTileRate = 0;
  #peakVideoRate = 0;
  #maxPendingCommands = 0;
  #maxBatchCommands = 0;

  constructor(handle: BrowserSessionHandle) {
    this.#handle = handle;
  }

  get active(): boolean {
    return this.#active;
  }

  get summary(): SessionPreviewMetricsSummary | null {
    return this.#summary;
  }

  start(nowMs: number): SessionPreviewMetricsSummary {
    const sample = requireRuntimeSample(this.#handle, nowMs);
    this.#baseline = baselineFromSample(sample);
    this.#lastSample = sample;
    this.#summary = this.#buildSummary(sample, 'running');
    this.#active = true;
    return this.#summary;
  }

  update(nowMs: number): SessionPreviewMetricsSummary {
    if (!this.#active || !this.#baseline) {
      throw new Error('Session preview metrics sample is not running.');
    }
    const sample = requireRuntimeSample(this.#handle, nowMs);
    this.#updatePeaks(sample);
    this.#summary = this.#buildSummary(sample, 'running');
    this.#lastSample = sample;
    return this.#summary;
  }

  stop(nowMs: number): SessionPreviewMetricsSummary | null {
    if (!this.#active || !this.#baseline) {
      this.#active = false;
      return this.#summary;
    }
    const sample = requireRuntimeSample(this.#handle, nowMs);
    this.#updatePeaks(sample);
    this.#summary = this.#buildSummary(sample, 'stopped');
    this.#lastSample = sample;
    this.#active = false;
    return this.#summary;
  }

  reset(): void {
    this.#baseline = null;
    this.#lastSample = null;
    this.#summary = null;
    this.#active = false;
    this.#peakRxRate = 0;
    this.#peakTxRate = 0;
    this.#peakTileRate = 0;
    this.#peakVideoRate = 0;
    this.#maxPendingCommands = 0;
    this.#maxBatchCommands = 0;
  }

  #updatePeaks(sample: RuntimeSample): void {
    this.#maxPendingCommands = Math.max(
      this.#maxPendingCommands,
      sample.tileRuntime.currentPendingCommands,
      sample.tileRuntime.pendingCommandsHighWaterMark,
    );
    this.#maxBatchCommands = Math.max(
      this.#maxBatchCommands,
      sample.tileRuntime.lastBatchCommands,
      sample.tileRuntime.maxBatchCommands,
    );
    if (!this.#lastSample) {
      return;
    }
    const durationSec = Math.max((sample.capturedAtMs - this.#lastSample.capturedAtMs) / 1000, 0.001);
    this.#peakRxRate = Math.max(
      this.#peakRxRate,
      positiveDelta(sample.sessionStats.transfer.rxBytes, this.#lastSample.sessionStats.transfer.rxBytes) / durationSec,
    );
    this.#peakTxRate = Math.max(
      this.#peakTxRate,
      positiveDelta(sample.sessionStats.transfer.txBytes, this.#lastSample.sessionStats.transfer.txBytes) / durationSec,
    );
    this.#peakTileRate = Math.max(
      this.#peakTileRate,
      positiveDelta(sample.sessionStats.tiles.commandBytes, this.#lastSample.sessionStats.tiles.commandBytes) / durationSec,
    );
    this.#peakVideoRate = Math.max(
      this.#peakVideoRate,
      positiveDelta(sample.sessionStats.video.datagramBytes, this.#lastSample.sessionStats.video.datagramBytes) / durationSec,
    );
  }

  #buildSummary(sample: RuntimeSample, status: SessionPreviewMetricsStatus): SessionPreviewMetricsSummary {
    const baseline = this.#baseline;
    if (!baseline) {
      throw new Error('Session preview metrics baseline is missing.');
    }
    const durationMs = Math.max(0, sample.capturedAtMs - baseline.capturedAtMs);
    const durationSec = Math.max(durationMs / 1000, 0.001);
    const scrollComposition = sample.sessionStats.tiles.scrollComposition;
    const scrollHealth = sample.sessionStats.tiles.scrollHealth;
    const scrollPotentialTiles = positiveDelta(scrollComposition.scrollPotentialTiles, baseline.scrollPotentialTiles);
    const scrollSavedTiles = positiveDelta(scrollComposition.scrollSavedTiles, baseline.scrollSavedTiles);
    const hostScrollBatches = positiveDelta(scrollHealth.hostScrollBatchesTotal, baseline.hostScrollBatches);
    const hostFallbacks = positiveDelta(scrollHealth.hostScrollFallbacksTotal, baseline.hostFallbacks);
    const hostFallbackReasons = {
      nonQuantizedFallbacks: positiveDelta(
        scrollHealth.hostScrollNonQuantizedFallbacksTotal,
        baseline.hostNonQuantizedFallbacks,
      ),
      residualFullRepaints: positiveDelta(
        scrollHealth.hostScrollResidualFullRepaintsTotal,
        baseline.hostResidualFullRepaints,
      ),
      residualInteriorLimitFallbacks: positiveDelta(
        scrollHealth.hostScrollResidualInteriorLimitFallbacksTotal,
        baseline.hostResidualInteriorLimitFallbacks,
      ),
      residualLowSavedRatioFallbacks: positiveDelta(
        scrollHealth.hostScrollResidualLowSavedRatioFallbacksTotal,
        baseline.hostResidualLowSavedRatioFallbacks,
      ),
      residualLargeRowShiftFallbacks: positiveDelta(
        scrollHealth.hostScrollResidualLargeRowShiftFallbacksTotal,
        baseline.hostResidualLargeRowShiftFallbacks,
      ),
      residualOtherFallbacks: positiveDelta(
        scrollHealth.hostScrollResidualOtherFallbacksTotal,
        baseline.hostResidualOtherFallbacks,
      ),
      zeroSavedBatches: positiveDelta(scrollHealth.hostScrollZeroSavedBatchesTotal, baseline.hostZeroSavedBatches),
    };
    const frameCount = positiveDelta(sample.frameCount, baseline.frameCount);
    const rxBytes = positiveDelta(sample.sessionStats.transfer.rxBytes, baseline.rxBytes);
    const txBytes = positiveDelta(sample.sessionStats.transfer.txBytes, baseline.txBytes);
    const tileBytes = positiveDelta(sample.sessionStats.tiles.commandBytes, baseline.tileBytes);
    const videoBytes = positiveDelta(sample.sessionStats.video.datagramBytes, baseline.videoBytes);

    return {
      status,
      durationMs,
      frameCount,
      averageFps: frameCount / durationSec,
      render: {
        backend: sample.render.backend,
        reason: sample.render.reason,
        renderer: sample.render.renderer ?? '',
        label: formatRenderDiagnostics(sample.render),
      },
      transfer: {
        rxBytes,
        txBytes,
        tileBytes,
        videoBytes,
        avgRxRate: rxBytes / durationSec,
        avgTxRate: txBytes / durationSec,
        avgTileRate: tileBytes / durationSec,
        avgVideoRate: videoBytes / durationSec,
        peakRxRate: this.#peakRxRate,
        peakTxRate: this.#peakTxRate,
        peakTileRate: this.#peakTileRate,
        peakVideoRate: this.#peakVideoRate,
      },
      tiles: {
        imageCommands: positiveDelta(sample.sessionStats.tiles.imageCommands, baseline.imageCommands),
        videoCommands: positiveDelta(sample.sessionStats.tiles.videoCommands, baseline.videoCommands),
        drawCommands: positiveDelta(sample.sessionStats.tiles.drawCommands, baseline.drawCommands),
        totalCommands: positiveDelta(sample.sessionStats.tiles.totalCommands, baseline.totalCommands),
        cacheHitRateObserved: sample.sessionStats.tiles.cacheHitRateObserved,
        cacheSizeObserved: sample.tileRuntime.size,
        cacheBytesObserved: sample.tileRuntime.bytes,
        cacheHits: positiveDelta(sample.tileRuntime.hits, baseline.cacheHits),
        cacheMisses: positiveDelta(sample.tileRuntime.misses, baseline.cacheMisses),
        cacheEvictions: positiveDelta(sample.tileRuntime.evictions, baseline.cacheEvictions),
        maxBatchCommands: Math.max(this.#maxBatchCommands, sample.tileRuntime.maxBatchCommands),
        maxPendingCommands: Math.max(this.#maxPendingCommands, sample.tileRuntime.pendingCommandsHighWaterMark),
      },
      scroll: {
        scrollBatches: positiveDelta(scrollComposition.scrollBatches, baseline.scrollBatches),
        savedRate: percent(scrollSavedTiles, scrollPotentialTiles),
        hostScrollBatches,
        hostFallbacks,
        hostFallbackRate: percent(hostFallbacks, hostScrollBatches),
        hostFallbackRateRecent20: scrollHealth.hostFallbackRateRecent20,
        hostFallbackRateRecent50: scrollHealth.hostFallbackRateRecent50,
        dominantHostFallbackReason: dominantHostFallbackReason(hostFallbackReasons),
        hostFallbackReasons,
      },
      video: {
        datagrams: positiveDelta(sample.sessionStats.video.datagrams, baseline.datagrams),
        droppedFrames: positiveDelta(sample.sessionStats.video.droppedFrames, baseline.droppedFrames),
      },
    };
  }
}

export function canSampleSessionPreviewMetrics(
  handle: BrowserSessionHandle | null,
): handle is BrowserSessionMetricsHandle {
  return Boolean(
    handle
      && typeof handle.getFrameCount === 'function'
      && typeof handle.getTileCacheStats === 'function'
      && typeof handle.getSessionStats === 'function'
      && typeof handle.getRenderDiagnostics === 'function',
  );
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) {
    return '0.0s';
  }
  return `${(ms / 1000).toFixed(1)}s`;
}

export function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatRenderDiagnostics(diagnostics: BrowserSessionRenderDiagnostics): string {
  const renderer = diagnostics.renderer
    ? diagnostics.renderer
        .replace(/^ANGLE \([^,]+,\s*/, '')
        .replace(/, Unspecified Version\)$/, '')
    : '';
  return `${diagnostics.backend} (${diagnostics.reason})${renderer ? ` ${renderer}` : ''}`;
}

function requireRuntimeSample(handle: BrowserSessionHandle, nowMs: number): RuntimeSample {
  if (!canSampleSessionPreviewMetrics(handle)) {
    throw new Error('This BrowserPane SDK handle does not expose runtime metrics.');
  }
  return {
    capturedAtMs: nowMs,
    frameCount: handle.getFrameCount(),
    tileRuntime: handle.getTileCacheStats(),
    sessionStats: handle.getSessionStats(),
    render: handle.getRenderDiagnostics(),
  };
}

function baselineFromSample(sample: RuntimeSample): MetricsBaseline {
  return {
    capturedAtMs: sample.capturedAtMs,
    frameCount: sample.frameCount,
    rxBytes: sample.sessionStats.transfer.rxBytes,
    txBytes: sample.sessionStats.transfer.txBytes,
    tileBytes: sample.sessionStats.tiles.commandBytes,
    videoBytes: sample.sessionStats.video.datagramBytes,
    droppedFrames: sample.sessionStats.video.droppedFrames,
    datagrams: sample.sessionStats.video.datagrams,
    imageCommands: sample.sessionStats.tiles.imageCommands,
    videoCommands: sample.sessionStats.tiles.videoCommands,
    drawCommands: sample.sessionStats.tiles.drawCommands,
    totalCommands: sample.sessionStats.tiles.totalCommands,
    scrollBatches: sample.sessionStats.tiles.scrollComposition.scrollBatches,
    scrollPotentialTiles: sample.sessionStats.tiles.scrollComposition.scrollPotentialTiles,
    scrollSavedTiles: sample.sessionStats.tiles.scrollComposition.scrollSavedTiles,
    hostScrollBatches: sample.sessionStats.tiles.scrollHealth.hostScrollBatchesTotal,
    hostFallbacks: sample.sessionStats.tiles.scrollHealth.hostScrollFallbacksTotal,
    hostNonQuantizedFallbacks: sample.sessionStats.tiles.scrollHealth.hostScrollNonQuantizedFallbacksTotal,
    hostResidualFullRepaints: sample.sessionStats.tiles.scrollHealth.hostScrollResidualFullRepaintsTotal,
    hostResidualInteriorLimitFallbacks:
      sample.sessionStats.tiles.scrollHealth.hostScrollResidualInteriorLimitFallbacksTotal,
    hostResidualLowSavedRatioFallbacks:
      sample.sessionStats.tiles.scrollHealth.hostScrollResidualLowSavedRatioFallbacksTotal,
    hostResidualLargeRowShiftFallbacks:
      sample.sessionStats.tiles.scrollHealth.hostScrollResidualLargeRowShiftFallbacksTotal,
    hostResidualOtherFallbacks: sample.sessionStats.tiles.scrollHealth.hostScrollResidualOtherFallbacksTotal,
    hostZeroSavedBatches: sample.sessionStats.tiles.scrollHealth.hostScrollZeroSavedBatchesTotal,
    cacheHits: sample.tileRuntime.hits,
    cacheMisses: sample.tileRuntime.misses,
    cacheEvictions: sample.tileRuntime.evictions,
  };
}

function positiveDelta(current: number, baseline: number): number {
  return Math.max(0, current - baseline);
}

function percent(numerator: number, denominator: number): number {
  if (denominator <= 0) {
    return 0;
  }
  return (numerator / denominator) * 100;
}

function dominantHostFallbackReason(
  reasonCounts: SessionPreviewMetricsSummary['scroll']['hostFallbackReasons'],
): string {
  const entries = [
    ['non-quantized', reasonCounts.nonQuantizedFallbacks],
    ['residual-full-repaint', reasonCounts.residualFullRepaints],
    ['residual-interior-limit', reasonCounts.residualInteriorLimitFallbacks],
    ['residual-low-saved-ratio', reasonCounts.residualLowSavedRatioFallbacks],
    ['residual-large-row-shift', reasonCounts.residualLargeRowShiftFallbacks],
    ['residual-other', reasonCounts.residualOtherFallbacks],
    ['zero-saved', reasonCounts.zeroSavedBatches],
  ] as const;
  let bestLabel = 'none';
  let bestValue = 0;
  for (const [label, value] of entries) {
    if (value > bestValue) {
      bestLabel = label;
      bestValue = value;
    }
  }
  return bestLabel;
}
