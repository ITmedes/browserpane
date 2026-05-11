import type { MetricsRawSample } from './metrics-diagnostics-payload';

export type MetricsSampleExtrema = {
  readonly peakRxRate: number;
  readonly peakTxRate: number;
  readonly peakTileRate: number;
  readonly peakVideoRate: number;
  readonly maxBatchCommands: number;
  readonly maxPendingCommands: number;
};

export class MetricsSampleExtremaBuilder {
  static initial(): MetricsSampleExtrema {
    return {
      peakRxRate: 0,
      peakTxRate: 0,
      peakTileRate: 0,
      peakVideoRate: 0,
      maxBatchCommands: 0,
      maxPendingCommands: 0,
    };
  }

  static next(
    current: MetricsSampleExtrema,
    previous: MetricsRawSample,
    sample: MetricsRawSample,
  ): MetricsSampleExtrema {
    const durationSec = Math.max((sample.capturedAtMs - previous.capturedAtMs) / 1000, 0.001);
    return {
      peakRxRate: Math.max(current.peakRxRate, rate(previous.stats.transfer?.rxBytes, sample.stats.transfer?.rxBytes, durationSec)),
      peakTxRate: Math.max(current.peakTxRate, rate(previous.stats.transfer?.txBytes, sample.stats.transfer?.txBytes, durationSec)),
      peakTileRate: Math.max(current.peakTileRate, rate(previous.stats.tiles?.commandBytes, sample.stats.tiles?.commandBytes, durationSec)),
      peakVideoRate: Math.max(current.peakVideoRate, rate(previous.stats.video?.datagramBytes, sample.stats.video?.datagramBytes, durationSec)),
      maxBatchCommands: Math.max(current.maxBatchCommands, numberValue(sample.tileCache?.lastBatchCommands), numberValue(sample.tileCache?.maxBatchCommands)),
      maxPendingCommands: Math.max(current.maxPendingCommands, numberValue(sample.tileCache?.currentPendingCommands), numberValue(sample.tileCache?.pendingCommandsHighWaterMark)),
    };
  }
}

function rate(start: number | undefined, end: number | undefined, durationSec: number): number {
  return Math.max(0, numberValue(end) - numberValue(start)) / durationSec;
}

function numberValue(value: number | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}
