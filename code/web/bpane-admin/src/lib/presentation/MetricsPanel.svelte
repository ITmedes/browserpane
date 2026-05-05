<script lang="ts">
  import type { MetricsViewModel } from './metrics-view-model';

  type MetricsPanelProps = {
    readonly viewModel: MetricsViewModel;
    readonly copied: boolean;
    readonly onStart: () => void;
    readonly onStop: () => void;
    readonly onCopy: () => void;
    readonly onReset: () => void;
  };

  let { viewModel, copied, onStart, onStop, onCopy, onReset }: MetricsPanelProps = $props();
  const rows = $derived([
    ['sample', viewModel.sample],
    ['render', viewModel.render],
    ['throughput', viewModel.throughput],
    ['tiles', viewModel.tiles],
    ['scroll', viewModel.scroll],
    ['video', viewModel.video],
  ]);
</script>

<section class="grid gap-4" aria-label="Metrics controls">
  <p class="m-0 text-sm leading-normal text-admin-ink/68">{viewModel.note}</p>

  <div class="grid grid-cols-2 gap-2 max-[760px]:grid-cols-1">
    {#each rows as row}
      <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
        {row[0]}
        <strong class="mt-1 block text-admin-ink normal-case" data-testid={`metrics-${row[0]}`}>{row[1]}</strong>
      </span>
    {/each}
  </div>

  <div class="flex flex-wrap gap-2">
    <button class="admin-button-primary" type="button" data-testid="metrics-start" disabled={!viewModel.canStart} onclick={onStart}>
      Start sample
    </button>
    <button class="admin-button-primary" type="button" data-testid="metrics-stop" disabled={!viewModel.canStop} onclick={onStop}>
      Stop sample
    </button>
    <button class="admin-button-primary" type="button" data-testid="metrics-copy" disabled={!viewModel.canCopy} onclick={onCopy}>
      {copied ? 'Copied metrics' : 'Copy metrics'}
    </button>
    <button class="admin-button-primary" type="button" data-testid="metrics-reset" onclick={onReset}>
      Reset metrics
    </button>
  </div>
</section>
