import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
import {
  MetricsDiagnosticsPayloadBuilder,
  type MetricsRawSample,
} from './metrics-diagnostics-payload';

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
    const payload = MetricsDiagnosticsPayloadBuilder.build(start, end);
    return {
      sample: `${payload.frames.delta} frames - ${payload.frames.fps.toFixed(1)} fps - ${formatDuration(payload.timing.durationMs)}`,
      render: `${end.diagnostics?.backend ?? '--'} - ${end.diagnostics?.reason ?? '--'}`,
      throughput: `down ${formatRate(payload.transfer.rxBytes, payload.timing.durationMs)} - up ${formatRate(payload.transfer.txBytes, payload.timing.durationMs)}`,
      tiles: `${payload.tiles.totalCommands} commands - ${formatBytes(payload.tiles.commandBytes)} tile data`,
      scroll: `${payload.scroll.batches} batches - host fallback ${payload.scroll.hostFallbackRate.toFixed(1)}%`,
      video: `${payload.video.datagrams} datagrams - ${formatBytes(payload.video.datagramBytes)}`,
      payload,
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

function formatRate(bytes: number, durationMs: number): string {
  const durationSec = Math.max(durationMs / 1000, 0.001);
  return `${formatBytes(bytes / durationSec)}/s`;
}
