<script lang="ts">
  import MetricsPanel from '../presentation/MetricsPanel.svelte';
  import {
    MetricsSampleSummaryBuilder,
    MetricsViewModelBuilder,
    type MetricsRawSample,
    type MetricsSampleSummary,
  } from '../presentation/metrics-view-model';
  import type { LiveBrowserSessionConnection } from '../session/browser-session-types';

  type MetricsSurfaceProps = {
    readonly liveConnection: LiveBrowserSessionConnection | null;
  };

  let { liveConnection }: MetricsSurfaceProps = $props();
  let active = $state(false);
  let startSample = $state<MetricsRawSample | null>(null);
  let summary = $state<MetricsSampleSummary | null>(null);
  let copied = $state(false);
  const viewModel = $derived(MetricsViewModelBuilder.build({ liveConnection, active, summary }));

  function start(): void {
    startSample = captureSample();
    summary = null;
    active = true;
    copied = false;
  }

  function stop(): void {
    if (!startSample) {
      return;
    }
    summary = MetricsSampleSummaryBuilder.fromSamples(startSample, captureSample());
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
    summary = null;
    copied = false;
  }

  function captureSample(): MetricsRawSample {
    const handle = liveConnection?.handle;
    return {
      capturedAtMs: performance.now(),
      frameCount: handle?.getFrameCount?.() ?? 0,
      stats: handle?.getSessionStats?.() ?? {},
      diagnostics: handle?.getRenderDiagnostics?.() ?? null,
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
