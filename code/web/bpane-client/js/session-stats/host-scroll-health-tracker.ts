import type { HostScrollHealthSnapshot } from './models.js';

type HostScrollHistorySample = {
  batches: number;
  fallbacks: number;
};

export class HostScrollHealthTracker {
  private static readonly HISTORY_MAX_BATCHES = 256;

  private readonly nowProvider: () => number;

  private initialized = false;

  private readonly totals: {
    hostScrollBatchesTotal: number;
    hostScrollFallbacksTotal: number;
    hostScrollPotentialTilesTotal: number;
    hostScrollSavedTilesTotal: number;
    hostScrollNonQuantizedFallbacksTotal: number;
    hostScrollResidualFullRepaintsTotal: number;
    hostScrollResidualInteriorLimitFallbacksTotal: number;
    hostScrollResidualLowSavedRatioFallbacksTotal: number;
    hostScrollResidualLargeRowShiftFallbacksTotal: number;
    hostScrollResidualOtherFallbacksTotal: number;
    hostScrollZeroSavedBatchesTotal: number;
    hostScrollSplitRegionBatchesTotal: number;
    hostScrollStickyBandBatchesTotal: number;
    hostScrollChromeTilesTotal: number;
    hostScrollExposedStripTilesTotal: number;
    hostScrollInteriorResidualTilesTotal: number;
    hostScrollEdgeStripResidualTilesTotal: number;
    hostScrollSmallEdgeStripResidualTilesTotal: number;
    hostScrollSmallEdgeStripResidualRowsTotal: number;
    hostScrollSmallEdgeStripResidualAreaPxTotal: number;
    hostSentHashEntries: number;
    hostSentHashEvictionsTotal: number;
    hostCacheMissReportsTotal: number;
    lastHostScrollStatsAtMs: number;
  } = {
    hostScrollBatchesTotal: 0,
    hostScrollFallbacksTotal: 0,
    hostScrollPotentialTilesTotal: 0,
    hostScrollSavedTilesTotal: 0,
    hostScrollNonQuantizedFallbacksTotal: 0,
    hostScrollResidualFullRepaintsTotal: 0,
    hostScrollResidualInteriorLimitFallbacksTotal: 0,
    hostScrollResidualLowSavedRatioFallbacksTotal: 0,
    hostScrollResidualLargeRowShiftFallbacksTotal: 0,
    hostScrollResidualOtherFallbacksTotal: 0,
    hostScrollZeroSavedBatchesTotal: 0,
    hostScrollSplitRegionBatchesTotal: 0,
    hostScrollStickyBandBatchesTotal: 0,
    hostScrollChromeTilesTotal: 0,
    hostScrollExposedStripTilesTotal: 0,
    hostScrollInteriorResidualTilesTotal: 0,
    hostScrollEdgeStripResidualTilesTotal: 0,
    hostScrollSmallEdgeStripResidualTilesTotal: 0,
    hostScrollSmallEdgeStripResidualRowsTotal: 0,
    hostScrollSmallEdgeStripResidualAreaPxTotal: 0,
    hostSentHashEntries: 0,
    hostSentHashEvictionsTotal: 0,
    hostCacheMissReportsTotal: 0,
    lastHostScrollStatsAtMs: 0,
  };

  private recentHistory: HostScrollHistorySample[] = [];

  constructor(nowProvider: () => number = () => performance.now()) {
    this.nowProvider = nowProvider;
  }

  record(
    hostScrollBatchesTotal: number,
    hostScrollFallbacksTotal: number,
    hostScrollPotentialTilesTotal: number,
    hostScrollSavedTilesTotal: number,
    hostScrollNonQuantizedFallbacksTotal: number,
    hostScrollResidualFullRepaintsTotal: number,
    hostScrollResidualInteriorLimitFallbacksTotal: number,
    hostScrollResidualLowSavedRatioFallbacksTotal: number,
    hostScrollResidualLargeRowShiftFallbacksTotal: number,
    hostScrollResidualOtherFallbacksTotal: number,
    hostScrollZeroSavedBatchesTotal: number,
    hostScrollSplitRegionBatchesTotal: number,
    hostScrollStickyBandBatchesTotal: number,
    hostScrollChromeTilesTotal: number,
    hostScrollExposedStripTilesTotal: number,
    hostScrollInteriorResidualTilesTotal: number,
    hostScrollEdgeStripResidualTilesTotal = 0,
    hostScrollSmallEdgeStripResidualTilesTotal = 0,
    hostScrollSmallEdgeStripResidualRowsTotal = 0,
    hostScrollSmallEdgeStripResidualAreaPxTotal = 0,
    hostSentHashEntries = 0,
    hostSentHashEvictionsTotal = 0,
    hostCacheMissReportsTotal = 0,
  ): void {
    const prevBatches = this.totals.hostScrollBatchesTotal;
    const prevFallbacks = this.totals.hostScrollFallbacksTotal;
    const prevPotential = this.totals.hostScrollPotentialTilesTotal;
    const prevSaved = this.totals.hostScrollSavedTilesTotal;
    const prevNonQuantized = this.totals.hostScrollNonQuantizedFallbacksTotal;
    const prevResidualFullRepaints = this.totals.hostScrollResidualFullRepaintsTotal;
    const prevResidualInteriorLimit = this.totals.hostScrollResidualInteriorLimitFallbacksTotal;
    const prevResidualLowSavedRatio = this.totals.hostScrollResidualLowSavedRatioFallbacksTotal;
    const prevResidualLargeRowShift = this.totals.hostScrollResidualLargeRowShiftFallbacksTotal;
    const prevResidualOther = this.totals.hostScrollResidualOtherFallbacksTotal;
    const prevZeroSaved = this.totals.hostScrollZeroSavedBatchesTotal;
    const prevSplitRegionBatches = this.totals.hostScrollSplitRegionBatchesTotal;
    const prevStickyBandBatches = this.totals.hostScrollStickyBandBatchesTotal;
    const prevChromeTiles = this.totals.hostScrollChromeTilesTotal;
    const prevExposedStripTiles = this.totals.hostScrollExposedStripTilesTotal;
    const prevInteriorResidualTiles = this.totals.hostScrollInteriorResidualTilesTotal;
    const prevEdgeStripResidualTiles = this.totals.hostScrollEdgeStripResidualTilesTotal;
    const prevSmallEdgeStripResidualTiles = this.totals.hostScrollSmallEdgeStripResidualTilesTotal;
    const prevSmallEdgeStripResidualRows = this.totals.hostScrollSmallEdgeStripResidualRowsTotal;
    const prevSmallEdgeStripResidualAreaPx =
      this.totals.hostScrollSmallEdgeStripResidualAreaPxTotal;
    const prevSentHashEvictions = this.totals.hostSentHashEvictionsTotal;
    const prevCacheMissReports = this.totals.hostCacheMissReportsTotal;

    if (
      !this.initialized
      || hostScrollBatchesTotal < prevBatches
      || hostScrollFallbacksTotal < prevFallbacks
      || hostScrollPotentialTilesTotal < prevPotential
      || hostScrollSavedTilesTotal < prevSaved
      || hostScrollNonQuantizedFallbacksTotal < prevNonQuantized
      || hostScrollResidualFullRepaintsTotal < prevResidualFullRepaints
      || hostScrollResidualInteriorLimitFallbacksTotal < prevResidualInteriorLimit
      || hostScrollResidualLowSavedRatioFallbacksTotal < prevResidualLowSavedRatio
      || hostScrollResidualLargeRowShiftFallbacksTotal < prevResidualLargeRowShift
      || hostScrollResidualOtherFallbacksTotal < prevResidualOther
      || hostScrollZeroSavedBatchesTotal < prevZeroSaved
      || hostScrollSplitRegionBatchesTotal < prevSplitRegionBatches
      || hostScrollStickyBandBatchesTotal < prevStickyBandBatches
      || hostScrollChromeTilesTotal < prevChromeTiles
      || hostScrollExposedStripTilesTotal < prevExposedStripTiles
      || hostScrollInteriorResidualTilesTotal < prevInteriorResidualTiles
      || hostScrollEdgeStripResidualTilesTotal < prevEdgeStripResidualTiles
      || hostScrollSmallEdgeStripResidualTilesTotal < prevSmallEdgeStripResidualTiles
      || hostScrollSmallEdgeStripResidualRowsTotal < prevSmallEdgeStripResidualRows
      || hostScrollSmallEdgeStripResidualAreaPxTotal < prevSmallEdgeStripResidualAreaPx
      || hostSentHashEvictionsTotal < prevSentHashEvictions
      || hostCacheMissReportsTotal < prevCacheMissReports
    ) {
      this.recentHistory = [];
      this.initialized = true;
    } else {
      const deltaBatches = hostScrollBatchesTotal - prevBatches;
      const deltaFallbacks = hostScrollFallbacksTotal - prevFallbacks;
      if (deltaBatches > 0 || deltaFallbacks > 0) {
        this.recentHistory.push({
          batches: Math.max(0, deltaBatches),
          fallbacks: Math.max(0, deltaFallbacks),
        });
        this.pruneRecentHistory();
      }
    }

    this.totals.hostScrollBatchesTotal = hostScrollBatchesTotal;
    this.totals.hostScrollFallbacksTotal = hostScrollFallbacksTotal;
    this.totals.hostScrollPotentialTilesTotal = hostScrollPotentialTilesTotal;
    this.totals.hostScrollSavedTilesTotal = hostScrollSavedTilesTotal;
    this.totals.hostScrollNonQuantizedFallbacksTotal = hostScrollNonQuantizedFallbacksTotal;
    this.totals.hostScrollResidualFullRepaintsTotal = hostScrollResidualFullRepaintsTotal;
    this.totals.hostScrollResidualInteriorLimitFallbacksTotal =
      hostScrollResidualInteriorLimitFallbacksTotal;
    this.totals.hostScrollResidualLowSavedRatioFallbacksTotal =
      hostScrollResidualLowSavedRatioFallbacksTotal;
    this.totals.hostScrollResidualLargeRowShiftFallbacksTotal =
      hostScrollResidualLargeRowShiftFallbacksTotal;
    this.totals.hostScrollResidualOtherFallbacksTotal = hostScrollResidualOtherFallbacksTotal;
    this.totals.hostScrollZeroSavedBatchesTotal = hostScrollZeroSavedBatchesTotal;
    this.totals.hostScrollSplitRegionBatchesTotal = hostScrollSplitRegionBatchesTotal;
    this.totals.hostScrollStickyBandBatchesTotal = hostScrollStickyBandBatchesTotal;
    this.totals.hostScrollChromeTilesTotal = hostScrollChromeTilesTotal;
    this.totals.hostScrollExposedStripTilesTotal = hostScrollExposedStripTilesTotal;
    this.totals.hostScrollInteriorResidualTilesTotal = hostScrollInteriorResidualTilesTotal;
    this.totals.hostScrollEdgeStripResidualTilesTotal = hostScrollEdgeStripResidualTilesTotal;
    this.totals.hostScrollSmallEdgeStripResidualTilesTotal =
      hostScrollSmallEdgeStripResidualTilesTotal;
    this.totals.hostScrollSmallEdgeStripResidualRowsTotal =
      hostScrollSmallEdgeStripResidualRowsTotal;
    this.totals.hostScrollSmallEdgeStripResidualAreaPxTotal =
      hostScrollSmallEdgeStripResidualAreaPxTotal;
    this.totals.hostSentHashEntries = hostSentHashEntries;
    this.totals.hostSentHashEvictionsTotal = hostSentHashEvictionsTotal;
    this.totals.hostCacheMissReportsTotal = hostCacheMissReportsTotal;
    this.totals.lastHostScrollStatsAtMs = this.nowProvider();
  }

  snapshot(): HostScrollHealthSnapshot {
    const recent20 = this.recentFallbackRate(20);
    const recent50 = this.recentFallbackRate(50);
    const hostFallbackRate = this.totals.hostScrollBatchesTotal > 0
      ? (this.totals.hostScrollFallbacksTotal / this.totals.hostScrollBatchesTotal) * 100
      : 0;
    const hostScrollSavedRate = this.totals.hostScrollPotentialTilesTotal > 0
      ? (this.totals.hostScrollSavedTilesTotal / this.totals.hostScrollPotentialTilesTotal) * 100
      : 0;

    return {
      hostScrollBatchesTotal: this.totals.hostScrollBatchesTotal,
      hostScrollFallbacksTotal: this.totals.hostScrollFallbacksTotal,
      hostScrollNonQuantizedFallbacksTotal: this.totals.hostScrollNonQuantizedFallbacksTotal,
      hostScrollResidualFullRepaintsTotal: this.totals.hostScrollResidualFullRepaintsTotal,
      hostScrollResidualInteriorLimitFallbacksTotal:
        this.totals.hostScrollResidualInteriorLimitFallbacksTotal,
      hostScrollResidualLowSavedRatioFallbacksTotal:
        this.totals.hostScrollResidualLowSavedRatioFallbacksTotal,
      hostScrollResidualLargeRowShiftFallbacksTotal:
        this.totals.hostScrollResidualLargeRowShiftFallbacksTotal,
      hostScrollResidualOtherFallbacksTotal: this.totals.hostScrollResidualOtherFallbacksTotal,
      hostScrollZeroSavedBatchesTotal: this.totals.hostScrollZeroSavedBatchesTotal,
      hostScrollSplitRegionBatchesTotal: this.totals.hostScrollSplitRegionBatchesTotal,
      hostScrollStickyBandBatchesTotal: this.totals.hostScrollStickyBandBatchesTotal,
      hostScrollChromeTilesTotal: this.totals.hostScrollChromeTilesTotal,
      hostScrollExposedStripTilesTotal: this.totals.hostScrollExposedStripTilesTotal,
      hostScrollInteriorResidualTilesTotal: this.totals.hostScrollInteriorResidualTilesTotal,
      hostScrollEdgeStripResidualTilesTotal: this.totals.hostScrollEdgeStripResidualTilesTotal,
      hostScrollSmallEdgeStripResidualTilesTotal:
        this.totals.hostScrollSmallEdgeStripResidualTilesTotal,
      hostScrollSmallEdgeStripResidualRowsTotal:
        this.totals.hostScrollSmallEdgeStripResidualRowsTotal,
      hostScrollSmallEdgeStripResidualAreaPxTotal:
        this.totals.hostScrollSmallEdgeStripResidualAreaPxTotal,
      hostSentHashEntries: this.totals.hostSentHashEntries,
      hostSentHashEvictionsTotal: this.totals.hostSentHashEvictionsTotal,
      hostCacheMissReportsTotal: this.totals.hostCacheMissReportsTotal,
      hostFallbackRate,
      hostFallbackRateRecent20: recent20.rate,
      hostFallbackRateRecent50: recent50.rate,
      hostFallbackRateRecent20Batches: recent20.batches,
      hostFallbackRateRecent50Batches: recent50.batches,
      hostScrollPotentialTilesTotal: this.totals.hostScrollPotentialTilesTotal,
      hostScrollSavedTilesTotal: this.totals.hostScrollSavedTilesTotal,
      hostScrollSavedRate,
      lastHostScrollStatsAtMs: this.totals.lastHostScrollStatsAtMs,
    };
  }

  private pruneRecentHistory(): void {
    let retainedBatches = 0;
    let keepFrom = this.recentHistory.length;

    while (keepFrom > 0) {
      const sample = this.recentHistory[keepFrom - 1];
      retainedBatches += sample.batches;
      if (retainedBatches > HostScrollHealthTracker.HISTORY_MAX_BATCHES) {
        break;
      }
      keepFrom -= 1;
    }

    if (keepFrom > 0) {
      this.recentHistory = this.recentHistory.slice(keepFrom);
    }
  }

  private recentFallbackRate(windowBatches: number): { rate: number; batches: number } {
    let remaining = Math.max(0, windowBatches);
    let batches = 0;
    let fallbacks = 0;

    for (let index = this.recentHistory.length - 1; index >= 0 && remaining > 0; index -= 1) {
      const sample = this.recentHistory[index];
      if (sample.batches <= 0) {
        continue;
      }

      const takenBatches = Math.min(remaining, sample.batches);
      const takenFallbacks = takenBatches === sample.batches
        ? sample.fallbacks
        : (sample.fallbacks * takenBatches) / sample.batches;

      batches += takenBatches;
      fallbacks += takenFallbacks;
      remaining -= takenBatches;
    }

    return {
      rate: batches > 0 ? (fallbacks / batches) * 100 : 0,
      batches,
    };
  }
}
