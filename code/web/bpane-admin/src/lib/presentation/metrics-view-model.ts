import type {
  BrowserSessionRenderDiagnostics,
  BrowserSessionStatsSnapshot,
  LiveBrowserSessionConnection,
} from '../session/browser-session-types';

export type MetricsRawSample = {
  readonly capturedAtMs: number;
  readonly frameCount: number;
  readonly stats: BrowserSessionStatsSnapshot;
  readonly diagnostics: BrowserSessionRenderDiagnostics | null;
};

export type MetricsSampleSummary = {
  readonly sample: string;
  readonly render: string;
  readonly throughput: string;
  readonly tiles: string;
  readonly scroll: string;
  readonly video: string;
  readonly payload: Readonly<Record<string, unknown>>;
};

export type MetricsViewModel = {
  readonly note: string;
  readonly sample: string;
  readonly render: string;
  readonly throughput: string;
  readonly tiles: string;
  readonly scroll: string;
  readonly video: string;
  readonly canStart: boolean;
  readonly canStop: boolean;
  readonly canCopy: boolean;
};

export class MetricsSampleSummaryBuilder {
  static fromSamples(start: MetricsRawSample, end: MetricsRawSample): MetricsSampleSummary {
    const durationMs = Math.max(0, end.capturedAtMs - start.capturedAtMs);
    const durationSec = Math.max(durationMs / 1000, 0.001);
    const frames = Math.max(0, end.frameCount - start.frameCount);
    const rxBytes = delta(end.stats.transfer?.rxBytes, start.stats.transfer?.rxBytes);
    const txBytes = delta(end.stats.transfer?.txBytes, start.stats.transfer?.txBytes);
    const tileBytes = delta(end.stats.tiles?.commandBytes, start.stats.tiles?.commandBytes);
    const videoBytes = delta(end.stats.video?.datagramBytes, start.stats.video?.datagramBytes);
    const videoDatagrams = delta(end.stats.video?.datagrams, start.stats.video?.datagrams);
    return {
      sample: `${frames} frames - ${(frames / durationSec).toFixed(1)} fps - ${formatDuration(durationMs)}`,
      render: `${end.diagnostics?.backend ?? '--'} - ${end.diagnostics?.reason ?? '--'}`,
      throughput: `down ${formatRate(rxBytes / durationSec)} - up ${formatRate(txBytes / durationSec)}`,
      tiles: `${delta(end.stats.tiles?.totalCommands, start.stats.tiles?.totalCommands)} commands - ${formatBytes(tileBytes)} tile data`,
      scroll: `${delta(end.stats.tiles?.scrollComposition?.scrollBatches, start.stats.tiles?.scrollComposition?.scrollBatches)} batches - host fallback ${(end.stats.tiles?.scrollHealth?.hostFallbackRate ?? 0).toFixed(1)}%`,
      video: `${videoDatagrams} datagrams - ${formatBytes(videoBytes)}`,
      payload: {
        durationMs,
        frames,
        fps: frames / durationSec,
        rxBytes,
        txBytes,
        tileBytes,
        videoBytes,
        videoDatagrams,
        renderDiagnostics: end.diagnostics,
      },
    };
  }
}

export class MetricsViewModelBuilder {
  static build(input: {
    readonly liveConnection: LiveBrowserSessionConnection | null;
    readonly active: boolean;
    readonly summary: MetricsSampleSummary | null;
  }): MetricsViewModel {
    const supported = Boolean(input.liveConnection?.handle.getSessionStats);
    return {
      note: supported
        ? 'Capture a before/after sample from the connected browser handle.'
        : 'Connect to a session with metrics-capable browser handle support.',
      sample: input.summary?.sample ?? (input.active ? 'running' : 'idle'),
      render: input.summary?.render ?? '--',
      throughput: input.summary?.throughput ?? '--',
      tiles: input.summary?.tiles ?? '--',
      scroll: input.summary?.scroll ?? '--',
      video: input.summary?.video ?? '--',
      canStart: supported && !input.active,
      canStop: supported && input.active,
      canCopy: Boolean(input.summary),
    };
  }
}

function delta(end: number | undefined, start: number | undefined): number {
  return Math.max(0, (end ?? 0) - (start ?? 0));
}

function formatDuration(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(1)} s`;
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}
