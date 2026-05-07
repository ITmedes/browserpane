import { describe, expect, it } from 'vitest';
import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
import { MetricsSampleSummaryBuilder, MetricsViewModelBuilder } from './metrics-view-model';

describe('MetricsViewModelBuilder', () => {
  it('enables sampling when the live handle exposes session stats', () => {
    const viewModel = MetricsViewModelBuilder.build({
      liveConnection: connection(),
      active: false,
      summary: null,
    });

    expect(viewModel.canStart).toBe(true);
    expect(viewModel.canStop).toBe(false);
    expect(viewModel.sample).toBe('idle');
  });

  it('summarizes sample deltas', () => {
    const summary = MetricsSampleSummaryBuilder.fromSamples({
      capturedAtMs: 1_000,
      frameCount: 10,
      stats: sampleStats({ rxBytes: 100, txBytes: 25, totalCommands: 2 }),
      diagnostics: renderDiagnostics(),
    }, {
      capturedAtMs: 2_000,
      frameCount: 40,
      stats: sampleStats({ rxBytes: 1_124, txBytes: 281, totalCommands: 9 }),
      diagnostics: renderDiagnostics(),
    });

    expect(summary.sample).toContain('30 frames');
    expect(summary.throughput).toContain('1.0 KB/s');
    expect(summary.tiles).toContain('7 commands');
  });

  it('builds a stable copyable diagnostics payload with deep counters', () => {
    const summary = MetricsSampleSummaryBuilder.fromSamples({
      capturedAtMs: 1_000,
      frameCount: 10,
      stats: sampleStats({
        rxBytes: 100,
        txBytes: 25,
        rxFrames: 3,
        txFrames: 1,
        totalCommands: 2,
        videoDatagrams: 1,
        videoDroppedFrames: 0,
      }),
      diagnostics: renderDiagnostics(),
    }, {
      capturedAtMs: 2_500,
      frameCount: 55,
      stats: sampleStats({
        rxBytes: 3_172,
        txBytes: 537,
        rxFrames: 19,
        txFrames: 7,
        commandBytes: 2_048,
        totalCommands: 17,
        cacheHits: 9,
        cacheMisses: 3,
        cacheSize: 12,
        scrollBatches: 4,
        scrollSavedTiles: 32,
        scrollPotentialTiles: 40,
        hostFallbackRate: 2.5,
        videoDatagrams: 8,
        videoDatagramBytes: 4_096,
        videoDroppedFrames: 2,
      }),
      diagnostics: renderDiagnostics(),
    });

    expect(summary.payload).toEqual(expect.objectContaining({
      schema: 'browserpane.admin.metrics.sample.v1',
      timing: expect.objectContaining({ durationMs: 1_500 }),
      frames: expect.objectContaining({ delta: 45, fps: 30 }),
      transfer: expect.objectContaining({ rxBytes: 3_072, txBytes: 512, rxFrames: 16, txFrames: 6 }),
      tiles: expect.objectContaining({
        commandBytes: 2_048,
        totalCommands: 15,
        cache: expect.objectContaining({ hits: 9, misses: 3, size: 12 }),
      }),
      scroll: expect.objectContaining({
        batches: 4,
        savedTiles: 32,
        potentialTiles: 40,
        hostFallbackRate: 2.5,
      }),
      video: expect.objectContaining({ datagrams: 7, datagramBytes: 4_096, droppedFrames: 2 }),
      render: renderDiagnostics(),
    }));
  });
});

function connection(): LiveBrowserSessionConnection {
  return {
    sessionId: 'session-a',
    gatewayUrl: 'https://localhost:4433/session',
    handle: {
      disconnect: () => {},
      getSessionStats: () => ({}),
    },
  };
}

function renderDiagnostics() {
  return { backend: 'webgl2', reason: 'ok', renderer: 'ANGLE', vendor: 'Google', software: false };
}

function sampleStats(input: {
  readonly rxBytes: number;
  readonly txBytes: number;
  readonly rxFrames?: number;
  readonly txFrames?: number;
  readonly commandBytes?: number;
  readonly totalCommands: number;
  readonly cacheHits?: number;
  readonly cacheMisses?: number;
  readonly cacheSize?: number;
  readonly scrollBatches?: number;
  readonly scrollSavedTiles?: number;
  readonly scrollPotentialTiles?: number;
  readonly hostFallbackRate?: number;
  readonly videoDatagrams?: number;
  readonly videoDatagramBytes?: number;
  readonly videoDroppedFrames?: number;
}) {
  return {
    transfer: {
      rxBytes: input.rxBytes,
      txBytes: input.txBytes,
      rxFrames: input.rxFrames ?? 0,
      txFrames: input.txFrames ?? 0,
    },
    tiles: {
      commandBytes: input.commandBytes ?? 0,
      totalCommands: input.totalCommands,
      cacheHitsObserved: input.cacheHits ?? 0,
      cacheMissesObserved: input.cacheMisses ?? 0,
      cacheSizeObserved: input.cacheSize ?? 0,
      scrollComposition: {
        scrollBatches: input.scrollBatches ?? 0,
        scrollSavedTiles: input.scrollSavedTiles ?? 0,
        scrollPotentialTiles: input.scrollPotentialTiles ?? 0,
      },
      scrollHealth: { hostFallbackRate: input.hostFallbackRate ?? 0 },
    },
    video: {
      datagrams: input.videoDatagrams ?? 0,
      datagramBytes: input.videoDatagramBytes ?? 0,
      droppedFrames: input.videoDroppedFrames ?? 0,
    },
  };
}
