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
      stats: { transfer: { rxBytes: 100, txBytes: 25 }, tiles: { commandBytes: 20, totalCommands: 2 } },
      diagnostics: { backend: 'webgl2', reason: 'ok' },
    }, {
      capturedAtMs: 2_000,
      frameCount: 40,
      stats: { transfer: { rxBytes: 1_124, txBytes: 281 }, tiles: { commandBytes: 532, totalCommands: 9 } },
      diagnostics: { backend: 'webgl2', reason: 'ok' },
    });

    expect(summary.sample).toContain('30 frames');
    expect(summary.throughput).toContain('1.0 KB/s');
    expect(summary.tiles).toContain('7 commands');
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
