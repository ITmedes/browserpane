import { describe, expect, it } from 'vitest';
import { HostScrollHealthTracker } from '../session-stats/host-scroll-health-tracker.js';

describe('HostScrollHealthTracker', () => {
  it('reports rolling fallback windows from cumulative totals', () => {
    let now = 1_000;
    const tracker = new HostScrollHealthTracker(() => now);

    tracker.record(0, 0, 0, 0);

    now = 1_100;
    tracker.record(10, 2, 100, 80);

    now = 1_200;
    tracker.record(30, 8, 300, 210);

    const snapshot = tracker.snapshot();

    expect(snapshot.hostFallbackRate).toBeCloseTo((8 / 30) * 100, 5);
    expect(snapshot.hostFallbackRateRecent20Batches).toBe(20);
    expect(snapshot.hostFallbackRateRecent20).toBeCloseTo(30, 5);
    expect(snapshot.hostFallbackRateRecent50Batches).toBe(30);
    expect(snapshot.hostFallbackRateRecent50).toBeCloseTo((8 / 30) * 100, 5);
    expect(snapshot.hostScrollSavedRate).toBeCloseTo((210 / 300) * 100, 5);
    expect(snapshot.lastHostScrollStatsAtMs).toBe(1_200);
  });

  it('clears rolling history when counters reset', () => {
    const tracker = new HostScrollHealthTracker();

    tracker.record(0, 0, 0, 0);
    tracker.record(12, 3, 120, 90);
    tracker.record(1, 0, 10, 10);

    const snapshot = tracker.snapshot();

    expect(snapshot.hostScrollBatchesTotal).toBe(1);
    expect(snapshot.hostScrollFallbacksTotal).toBe(0);
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
