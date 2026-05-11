import type { LiveBrowserSessionConnection } from '../session/browser-session-types';
import {
  MetricsDiagnosticsPayloadBuilder,
  type MetricsDiagnosticsPayload,
  type MetricsRawSample,
} from './metrics-diagnostics-payload';
import type { MetricsSampleExtrema } from './metrics-sample-extrema';

export type MetricsDetailItem = { readonly label: string; readonly value: string; readonly testId: string };
export type MetricsDetailSection = { readonly title: string; readonly items: readonly MetricsDetailItem[] };

export type MetricsSampleSummary = {
  readonly sample: string;
  readonly render: string;
  readonly throughput: string;
  readonly tiles: string;
  readonly scroll: string;
  readonly video: string;
  readonly details: readonly MetricsDetailSection[];
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
  readonly details: readonly MetricsDetailSection[];
  readonly canStart: boolean;
  readonly canStop: boolean;
  readonly canCopy: boolean;
};

export class MetricsSampleSummaryBuilder {
  static fromSamples(start: MetricsRawSample, end: MetricsRawSample, extrema?: MetricsSampleExtrema): MetricsSampleSummary {
    const payload = MetricsDiagnosticsPayloadBuilder.build(start, end, extrema);
    return {
      sample: `${payload.frames.delta} frames - ${payload.frames.fps.toFixed(1)} fps - ${formatDuration(payload.timing.durationMs)}`,
      render: renderLabel(payload.render),
      throughput: `down ${formatRateValue(payload.transfer.avgRxRate)} peak ${formatRateValue(payload.transfer.peakRxRate)} - up ${formatRateValue(payload.transfer.avgTxRate)}`,
      tiles: `${payload.tiles.totalCommands} commands - cache ${payload.tiles.cache.hitRate.toFixed(1)}% - ${formatBytes(payload.tiles.cache.bytes)}`,
      scroll: `${payload.scroll.batches} batches - saved ${payload.scroll.reuseRate.toFixed(1)}% - host fallback ${payload.scroll.hostFallbackRate.toFixed(1)}%`,
      video: `${payload.video.datagrams} datagrams - dropped ${payload.video.droppedFrames} - ${formatBytes(payload.video.datagramBytes)}`,
      details: detailSections(payload),
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
      sample: input.summary ? `${input.active ? 'running' : 'stopped'} - ${input.summary.sample}` : input.active ? 'running' : 'idle',
      render: input.summary?.render ?? '--',
      throughput: input.summary?.throughput ?? '--',
      tiles: input.summary?.tiles ?? '--',
      scroll: input.summary?.scroll ?? '--',
      video: input.summary?.video ?? '--',
      details: input.summary?.details ?? [],
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

function formatRateValue(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`;
}

function renderLabel(render: MetricsDiagnosticsPayload['render']): string {
  if (!render) {
    return '--';
  }
  const diagnostics = render as { backend?: string; reason?: string; renderer?: string | null; software?: boolean };
  const renderer = diagnostics.renderer ? ` ${diagnostics.renderer.replace(/^ANGLE \([^,]+,\s*/, '').replace(/, Unspecified Version\)$/, '')}` : '';
  return `${diagnostics.backend ?? '--'} - ${diagnostics.reason ?? '--'}${diagnostics.software ? ' - software' : ''}${renderer}`;
}

function detailSections(payload: ReturnType<typeof MetricsDiagnosticsPayloadBuilder.build>): readonly MetricsDetailSection[] {
  return [
    section('Transfer', [
      item('Received', `${formatBytes(payload.transfer.rxBytes)} / ${payload.transfer.rxFrames} frames`, 'metrics-detail-rx'),
      item('Sent', `${formatBytes(payload.transfer.txBytes)} / ${payload.transfer.txFrames} frames`, 'metrics-detail-tx'),
      item('Average rates', `down ${formatRateValue(payload.transfer.avgRxRate)} - up ${formatRateValue(payload.transfer.avgTxRate)}`, 'metrics-detail-avg-rates'),
      item('Tile/video rates', `tile ${formatRateValue(payload.transfer.avgTileRate)} - video ${formatRateValue(payload.transfer.avgVideoRate)}`, 'metrics-detail-media-rates'),
      item('Peak rates', `down ${formatRateValue(payload.transfer.peakRxRate)} - tile ${formatRateValue(payload.transfer.peakTileRate)}`, 'metrics-detail-peak-rates'),
    ]),
    section('Tiles', [
      item('Command mix', `${payload.tiles.imageCommands} image / ${payload.tiles.videoCommands} video / ${payload.tiles.drawCommands} draw`, 'metrics-detail-command-mix'),
      item('Cache', `${payload.tiles.cache.size} entries - ${payload.tiles.cache.hitRate.toFixed(1)}% hit - ${formatBytes(payload.tiles.cache.bytes)}`, 'metrics-detail-cache'),
      item('Evictions', `${payload.tiles.cache.evictions} evicted - ${formatBytes(payload.tiles.redundant.bytes)} redundant`, 'metrics-detail-evictions'),
      item('Batches', `${payload.tiles.batches.queued} queued - avg ${payload.tiles.batches.averageCommands.toFixed(1)} - max ${payload.tiles.batches.maxCommands}`, 'metrics-detail-batches'),
      item('Pending high water', `${payload.tiles.batches.maxPendingCommands} commands`, 'metrics-detail-pending'),
    ]),
    section('Scroll', [
      item('Reuse', `${payload.scroll.savedTiles}/${payload.scroll.potentialTiles} tiles - ${payload.scroll.reuseRate.toFixed(1)}%`, 'metrics-detail-scroll-reuse'),
      item('Sub-tile reuse', `${payload.scroll.subTileSavedTiles}/${payload.scroll.subTilePotentialTiles} tiles - ${payload.scroll.subTileReuseRate.toFixed(1)}%`, 'metrics-detail-subtile-reuse'),
      item('Host fallback', `${payload.scroll.hostFallbacks}/${payload.scroll.hostBatches} batches - ${payload.scroll.hostFallbackRate.toFixed(1)}%`, 'metrics-detail-host-fallback'),
      item('Dominant reason', payload.scroll.dominantHostFallbackReason, 'metrics-detail-host-reason'),
      item('Recent fallback', `20=${payload.scroll.hostFallbackRateRecent20.toFixed(1)}% / 50=${payload.scroll.hostFallbackRateRecent50.toFixed(1)}%`, 'metrics-detail-host-recent'),
    ]),
    section('Video', [
      item('Datagrams', `${payload.video.datagrams} datagrams - ${formatBytes(payload.video.datagramBytes)}`, 'metrics-detail-video-datagrams'),
      item('Frames', `${payload.video.decodedFrames} decoded - ${payload.video.droppedFrames} dropped`, 'metrics-detail-video-frames'),
      item('Payload split', `tiles ${formatBytes(payload.transfer.tileBytes)} - video ${formatBytes(payload.transfer.videoBytes)}`, 'metrics-detail-payload-split'),
    ]),
  ];
}

function section(title: string, items: readonly MetricsDetailItem[]): MetricsDetailSection {
  return { title, items };
}

function item(label: string, value: string, testId: string): MetricsDetailItem {
  return { label, value, testId };
}
