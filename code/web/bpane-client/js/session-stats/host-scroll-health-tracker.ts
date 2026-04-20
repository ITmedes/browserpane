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
    hostScrollZeroSavedBatchesTotal: number;
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
    hostScrollZeroSavedBatchesTotal: 0,
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
    hostScrollZeroSavedBatchesTotal: number,
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
    const prevZeroSaved = this.totals.hostScrollZeroSavedBatchesTotal;
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
      || hostScrollZeroSavedBatchesTotal < prevZeroSaved
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
    this.totals.hostScrollZeroSavedBatchesTotal = hostScrollZeroSavedBatchesTotal;
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
      hostScrollZeroSavedBatchesTotal: this.totals.hostScrollZeroSavedBatchesTotal,
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
