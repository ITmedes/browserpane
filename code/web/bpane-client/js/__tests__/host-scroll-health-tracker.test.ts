import { describe, expect, it } from 'vitest';
import { HostScrollHealthTracker } from '../session-stats/host-scroll-health-tracker.js';

describe('HostScrollHealthTracker', () => {
  it('reports rolling fallback windows from cumulative totals', () => {
    let now = 1_000;
    const tracker = new HostScrollHealthTracker(() => now);

    tracker.record(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);

    now = 1_100;
    tracker.record(10, 2, 100, 80, 1, 1, 1, 0, 0, 0, 2, 3, 1, 12, 4, 8);

    now = 1_200;
    tracker.record(30, 8, 300, 210, 3, 5, 2, 1, 1, 1, 4, 8, 3, 30, 10, 18);

    const snapshot = tracker.snapshot();

    expect(snapshot.hostFallbackRate).toBeCloseTo((8 / 30) * 100, 5);
    expect(snapshot.hostFallbackRateRecent20Batches).toBe(20);
    expect(snapshot.hostFallbackRateRecent20).toBeCloseTo(30, 5);
    expect(snapshot.hostFallbackRateRecent50Batches).toBe(30);
    expect(snapshot.hostFallbackRateRecent50).toBeCloseTo((8 / 30) * 100, 5);
    expect(snapshot.hostScrollNonQuantizedFallbacksTotal).toBe(3);
    expect(snapshot.hostScrollResidualFullRepaintsTotal).toBe(5);
    expect(snapshot.hostScrollResidualInteriorLimitFallbacksTotal).toBe(2);
    expect(snapshot.hostScrollResidualLowSavedRatioFallbacksTotal).toBe(1);
    expect(snapshot.hostScrollResidualLargeRowShiftFallbacksTotal).toBe(1);
    expect(snapshot.hostScrollResidualOtherFallbacksTotal).toBe(1);
    expect(snapshot.hostScrollZeroSavedBatchesTotal).toBe(4);
    expect(snapshot.hostScrollSplitRegionBatchesTotal).toBe(8);
    expect(snapshot.hostScrollStickyBandBatchesTotal).toBe(3);
    expect(snapshot.hostScrollChromeTilesTotal).toBe(30);
    expect(snapshot.hostScrollExposedStripTilesTotal).toBe(10);
    expect(snapshot.hostScrollInteriorResidualTilesTotal).toBe(18);
    expect(snapshot.hostScrollEdgeStripResidualTilesTotal).toBe(0);
    expect(snapshot.hostScrollSmallEdgeStripResidualTilesTotal).toBe(0);
    expect(snapshot.hostScrollSmallEdgeStripResidualRowsTotal).toBe(0);
    expect(snapshot.hostScrollSmallEdgeStripResidualAreaPxTotal).toBe(0);
    expect(snapshot.hostScrollSavedRate).toBeCloseTo((210 / 300) * 100, 5);
    expect(snapshot.lastHostScrollStatsAtMs).toBe(1_200);
  });

  it('clears rolling history when counters reset', () => {
    const tracker = new HostScrollHealthTracker();

    tracker.record(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    tracker.record(12, 3, 120, 90, 1, 2, 1, 1, 0, 0, 3, 4, 2, 18, 6, 11);
    tracker.record(1, 0, 10, 10, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0);

    const snapshot = tracker.snapshot();

    expect(snapshot.hostScrollBatchesTotal).toBe(1);
    expect(snapshot.hostScrollFallbacksTotal).toBe(0);
    expect(snapshot.hostScrollNonQuantizedFallbacksTotal).toBe(0);
    expect(snapshot.hostScrollResidualFullRepaintsTotal).toBe(0);
    expect(snapshot.hostScrollResidualInteriorLimitFallbacksTotal).toBe(0);
    expect(snapshot.hostScrollResidualLowSavedRatioFallbacksTotal).toBe(0);
    expect(snapshot.hostScrollResidualLargeRowShiftFallbacksTotal).toBe(0);
    expect(snapshot.hostScrollResidualOtherFallbacksTotal).toBe(0);
    expect(snapshot.hostScrollZeroSavedBatchesTotal).toBe(1);
    expect(snapshot.hostScrollEdgeStripResidualTilesTotal).toBe(0);
    expect(snapshot.hostScrollSmallEdgeStripResidualTilesTotal).toBe(0);
    expect(snapshot.hostScrollSmallEdgeStripResidualRowsTotal).toBe(0);
    expect(snapshot.hostScrollSmallEdgeStripResidualAreaPxTotal).toBe(0);
    expect(snapshot.hostFallbackRateRecent20Batches).toBe(0);
    expect(snapshot.hostFallbackRateRecent20).toBe(0);
    expect(snapshot.hostFallbackRateRecent50Batches).toBe(0);
    expect(snapshot.hostFallbackRateRecent50).toBe(0);
  });

  it('returns zero rates before any host scroll telemetry is recorded', () => {
    const tracker = new HostScrollHealthTracker();

    expect(tracker.snapshot()).toEqual({
      hostScrollBatchesTotal: 0,
      hostScrollFallbacksTotal: 0,
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
      hostFallbackRate: 0,
      hostFallbackRateRecent20: 0,
      hostFallbackRateRecent50: 0,
      hostFallbackRateRecent20Batches: 0,
      hostFallbackRateRecent50Batches: 0,
      hostScrollPotentialTilesTotal: 0,
      hostScrollSavedTilesTotal: 0,
      hostScrollSavedRate: 0,
      lastHostScrollStatsAtMs: 0,
    });
  });
});
