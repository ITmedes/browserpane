<script lang="ts">
  import MetricsPanel from '../presentation/MetricsPanel.svelte';
  import {
    MetricsSampleSummaryBuilder,
    MetricsViewModelBuilder,
    type MetricsSampleSummary,
  } from '../presentation/metrics-view-model';
  import type { MetricsRawSample } from '../presentation/metrics-diagnostics-payload';
  import { MetricsSampleExtremaBuilder, type MetricsSampleExtrema } from '../presentation/metrics-sample-extrema';
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';

  type MetricsSurfaceProps = {
    readonly liveConnection: LiveBrowserSessionConnection | null;
  };

  let { liveConnection }: MetricsSurfaceProps = $props();
  let active = $state(false);
  let startSample = $state<MetricsRawSample | null>(null);
  let previousSample = $state<MetricsRawSample | null>(null);
  let extrema = $state<MetricsSampleExtrema | null>(null);
  let summary = $state<MetricsSampleSummary | null>(null);
  let copied = $state(false);
  const viewModel = $derived(MetricsViewModelBuilder.build({ liveConnection, active, summary }));

  $effect(() => {
    if (!active) {
      return;
    }
    const interval = window.setInterval(updateActiveSummary, 1000);
    return () => window.clearInterval(interval);
  });

  function start(): void {
    startSample = captureSample();
    previousSample = startSample;
    extrema = MetricsSampleExtremaBuilder.initial();
    summary = MetricsSampleSummaryBuilder.fromSamples(startSample, startSample, extrema);
    active = true;
    copied = false;
  }

  function stop(): void {
    if (!startSample) {
      return;
    }
    updateActiveSummary();
    active = false;
  }

  async function copy(): Promise<void> {
    if (!summary) {
      return;
    }
    await navigator.clipboard?.writeText(JSON.stringify(summary.payload, null, 2));
    copied = true;
  }

  function reset(): void {
    active = false;
    startSample = null;
    previousSample = null;
    extrema = null;
    summary = null;
    copied = false;
  }

  function updateActiveSummary(): void {
    if (!startSample || !previousSample || !extrema) {
      return;
    }
    const sample = captureSample();
    extrema = MetricsSampleExtremaBuilder.next(extrema, previousSample, sample);
    previousSample = sample;
    summary = MetricsSampleSummaryBuilder.fromSamples(startSample, sample, extrema);
    copied = false;
  }

  function captureSample(): MetricsRawSample {
    const handle = liveConnection?.handle;
    return {
      capturedAtMs: performance.now(),
      frameCount: handle?.getFrameCount?.() ?? 0,
      stats: handle?.getSessionStats?.() ?? {},
      diagnostics: handle?.getRenderDiagnostics?.() ?? null,
      tileCache: handle?.getTileCacheStats?.() ?? null,
    };
  }
</script>

<MetricsPanel
  {viewModel}
  {copied}
  onStart={start}
  onStop={stop}
  onCopy={() => void copy()}
  onReset={reset}
/>
