<script lang="ts">
  import MetricsPanel from '../presentation/MetricsPanel.svelte';
  import {
    MetricsSampleSummaryBuilder,
    MetricsViewModelBuilder,
    type MetricsSampleSummary,
  } from '../presentation/metrics-view-model';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
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
  let feedback = $state<AdminMessageFeedback | null>(null);
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
    feedback = { variant: 'info', title: 'Metrics sampling', message: 'Metrics sample started.', testId: 'metrics-message' };
  }

  function stop(): void {
    if (!startSample) {
      return;
    }
    updateActiveSummary();
    active = false;
    feedback = { variant: 'success', title: 'Metrics sample ready', message: 'Metrics sample stopped and summary is ready.', testId: 'metrics-message' };
  }

  async function copy(): Promise<void> {
    if (!summary) {
      return;
    }
    try {
      await navigator.clipboard?.writeText(JSON.stringify(summary.payload, null, 2));
      copied = true;
      feedback = { variant: 'success', title: 'Metrics copied', message: 'Metrics diagnostics payload copied.', testId: 'metrics-message' };
    } catch (error) {
      feedback = {
        variant: 'error',
        title: 'Metrics copy failed',
        message: error instanceof Error ? error.message : 'Could not copy metrics diagnostics payload.',
        testId: 'metrics-message',
      };
    }
  }

  function reset(): void {
    active = false;
    startSample = null;
    previousSample = null;
    extrema = null;
    summary = null;
    copied = false;
    feedback = { variant: 'info', title: 'Metrics reset', message: 'Metrics sample state cleared.', testId: 'metrics-message' };
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
  {feedback}
  onStart={start}
  onStop={stop}
  onCopy={() => void copy()}
  onReset={reset}
/>
