<script lang="ts">
  import { onDestroy } from 'svelte';
  import { Activity, Clipboard, Play, RefreshCw, Square } from '@lucide/svelte';
  import type { LiveBrowserSessionConnection } from '$lib/session-preview/browser-session-types';
  import {
    canSampleSessionPreviewMetrics,
    formatBytes,
    formatDuration,
    formatRate,
    SessionPreviewMetricsSampler,
    type SessionPreviewMetricsSummary,
  } from '$lib/session-preview/session-preview-metrics';

  type SessionPreviewMetricsDrawerProps = {
    readonly connection: LiveBrowserSessionConnection | null;
  };

  let { connection }: SessionPreviewMetricsDrawerProps = $props();

  let open = $state(false);
  let summary = $state<SessionPreviewMetricsSummary | null>(null);
  let sampler = $state<SessionPreviewMetricsSampler | null>(null);
  let message = $state('');
  let connectionKey = $state('');
  let intervalId: ReturnType<typeof setInterval> | null = null;

  const canSample = $derived(canSampleSessionPreviewMetrics(connection?.handle ?? null));
  const running = $derived(sampler?.active ?? false);
  const rows = $derived(summaryRows(summary));

  onDestroy(stopTimer);

  $effect(() => {
    const nextKey = connection?.sessionId ?? '';
    if (nextKey === connectionKey) {
      return;
    }
    connectionKey = nextKey;
    resetSample();
  });

  function toggleOpen(): void {
    open = !open;
  }

  function startSample(): void {
    const handle = connection?.handle ?? null;
    if (!handle || !canSampleSessionPreviewMetrics(handle)) {
      message = 'Runtime metrics are not available for this preview connection.';
      return;
    }
    stopTimer();
    sampler = new SessionPreviewMetricsSampler(handle);
    summary = sampler.start(performance.now());
    message = '';
    intervalId = setInterval(updateSample, 1000);
  }

  function updateSample(): void {
    if (!sampler?.active) {
      stopTimer();
      return;
    }
    try {
      summary = sampler.update(performance.now());
      message = '';
    } catch (error) {
      stopTimer();
      message = error instanceof Error ? error.message : 'Runtime metrics update failed.';
    }
  }

  function stopSample(): void {
    stopTimer();
    if (!sampler) {
      return;
    }
    try {
      summary = sampler.stop(performance.now());
      message = '';
    } catch (error) {
      message = error instanceof Error ? error.message : 'Runtime metrics stop failed.';
    }
  }

  function resetSample(): void {
    stopTimer();
    sampler?.reset();
    sampler = null;
    summary = null;
    message = '';
  }

  async function copySummary(): Promise<void> {
    if (!summary) {
      return;
    }
    if (!navigator.clipboard?.writeText) {
      message = 'Clipboard access is not available in this browser context.';
      return;
    }
    try {
      await navigator.clipboard.writeText(JSON.stringify(summary, null, 2));
      message = 'Metrics copied.';
    } catch (error) {
      message = error instanceof Error ? error.message : 'Failed to copy metrics.';
    }
  }

  function stopTimer(): void {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  }

  function summaryRows(current: SessionPreviewMetricsSummary | null): readonly {
    readonly label: string;
    readonly value: string;
    readonly testId: string;
  }[] {
    if (!current) {
      return [];
    }
    return [
      {
        label: 'Sample',
        value: `${current.status} ${formatDuration(current.durationMs)} · ${current.averageFps.toFixed(1)} fps · ${current.frameCount} frames`,
        testId: 'session-preview-metrics-sample',
      },
      {
        label: 'Render',
        value: current.render.label,
        testId: 'session-preview-metrics-render',
      },
      {
        label: 'Transfer',
        value: `down ${formatRate(current.transfer.avgRxRate)} · up ${formatRate(current.transfer.avgTxRate)} · tile ${formatRate(current.transfer.avgTileRate)} · video ${formatRate(current.transfer.avgVideoRate)}`,
        testId: 'session-preview-metrics-transfer',
      },
      {
        label: 'Tiles',
        value: `${current.tiles.totalCommands} commands · image ${current.tiles.imageCommands} · video ${current.tiles.videoCommands} · draw ${current.tiles.drawCommands} · cache ${current.tiles.cacheHitRateObserved.toFixed(1)}% · ${current.tiles.cacheSizeObserved} entries · ${formatBytes(current.tiles.cacheBytesObserved)}`,
        testId: 'session-preview-metrics-tiles',
      },
      {
        label: 'Scroll',
        value: `${current.scroll.scrollBatches} batches · saved ${current.scroll.savedRate.toFixed(1)}% · fallback ${current.scroll.hostFallbackRate.toFixed(1)}% · recent ${current.scroll.hostFallbackRateRecent20.toFixed(1)}%/${current.scroll.hostFallbackRateRecent50.toFixed(1)}% · ${current.scroll.dominantHostFallbackReason}`,
        testId: 'session-preview-metrics-scroll',
      },
      {
        label: 'Video',
        value: `${current.video.datagrams} datagrams · ${current.video.droppedFrames} dropped · ${formatBytes(current.transfer.videoBytes)}`,
        testId: 'session-preview-metrics-video',
      },
    ];
  }
</script>

<div class="absolute right-4 top-4 z-20 flex max-w-[calc(100vw-2rem)] flex-col items-end gap-2" data-testid="session-preview-metrics">
  <button
    class="inline-flex h-9 items-center gap-2 rounded-md border border-white/10 bg-slate-900/90 px-3 text-xs font-semibold text-white shadow-lg shadow-black/30 backdrop-blur hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-50"
    type="button"
    onclick={toggleOpen}
    data-testid="session-preview-metrics-toggle"
  >
    <Activity size={14} strokeWidth={1.8} />
    <span>Metrics</span>
  </button>

  {#if open}
    <aside
      class="max-h-[calc(100vh-7rem)] w-[min(92vw,680px)] overflow-auto rounded-md border border-white/10 bg-slate-950/95 p-4 text-slate-100 shadow-2xl shadow-black/40 backdrop-blur"
      data-testid="session-preview-metrics-drawer"
    >
      <div class="flex flex-col gap-3 border-b border-white/10 pb-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <h2 class="m-0 text-sm font-semibold text-white">Transition metrics</h2>
          <p class="m-0 mt-1 text-xs leading-5 text-slate-400">
            {connection?.sessionId ?? 'No active preview connection'}
          </p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="inline-flex h-8 items-center gap-2 rounded-md border border-emerald-400/30 bg-emerald-500/15 px-3 text-xs font-semibold text-emerald-100 hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
            type="button"
            onclick={startSample}
            disabled={!connection || !canSample || running}
            data-testid="session-preview-metrics-start"
          >
            <Play size={13} strokeWidth={1.8} />
            <span>Start</span>
          </button>
          <button
            class="inline-flex h-8 items-center gap-2 rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-white hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
            type="button"
            onclick={stopSample}
            disabled={!running}
            data-testid="session-preview-metrics-stop"
          >
            <Square size={13} strokeWidth={1.8} />
            <span>Stop</span>
          </button>
          <button
            class="inline-flex h-8 items-center gap-2 rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-white hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
            type="button"
            onclick={resetSample}
            disabled={!summary && !running}
            data-testid="session-preview-metrics-reset"
          >
            <RefreshCw size={13} strokeWidth={1.8} />
            <span>Reset</span>
          </button>
          <button
            class="inline-flex h-8 items-center gap-2 rounded-md border border-white/10 bg-white/5 px-3 text-xs font-semibold text-white hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
            type="button"
            onclick={() => void copySummary()}
            disabled={!summary}
            data-testid="session-preview-metrics-copy"
          >
            <Clipboard size={13} strokeWidth={1.8} />
            <span>Copy</span>
          </button>
        </div>
      </div>

      {#if !connection}
        <p class="m-0 mt-3 rounded-md border border-white/10 bg-white/5 p-3 text-sm text-slate-300" data-testid="session-preview-metrics-empty">
          Connect the preview before sampling metrics.
        </p>
      {:else if !canSample}
        <p class="m-0 mt-3 rounded-md border border-amber-400/30 bg-amber-500/10 p-3 text-sm text-amber-100" data-testid="session-preview-metrics-unavailable">
          Runtime metrics are not available for this BrowserPane SDK handle.
        </p>
      {:else if rows.length === 0}
        <p class="m-0 mt-3 rounded-md border border-white/10 bg-white/5 p-3 text-sm text-slate-300" data-testid="session-preview-metrics-idle">
          Start a sample to capture transition metrics.
        </p>
      {:else}
        <dl class="mt-3 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-2 sm:grid-cols-2" data-testid="session-preview-metrics-grid">
          {#each rows as row}
            <div class="min-w-0 rounded-md border border-white/10 bg-white/5 p-3" data-testid={row.testId}>
              <dt class="text-[11px] font-semibold uppercase text-slate-400">{row.label}</dt>
              <dd class="m-0 mt-1 break-words text-sm leading-5 text-white">{row.value}</dd>
            </div>
          {/each}
        </dl>
      {/if}

      {#if message}
        <p class="m-0 mt-3 rounded-md border border-white/10 bg-white/5 p-3 text-xs text-slate-200" data-testid="session-preview-metrics-message">
          {message}
        </p>
      {/if}
    </aside>
  {/if}
</div>
