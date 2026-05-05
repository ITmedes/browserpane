<script lang="ts">
  import type { BrowserPolicySignal, BrowserPolicyViewModel } from './browser-policy-view-model';

  type BrowserPolicyPanelProps = {
    readonly viewModel: BrowserPolicyViewModel;
    readonly copied: boolean;
    readonly onRefresh: () => void;
    readonly onCopyProbeCommand: () => void;
  };

  let { viewModel, copied, onRefresh, onCopyProbeCommand }: BrowserPolicyPanelProps = $props();

  function signalClass(signal: BrowserPolicySignal): string {
    if (signal.tone === 'ok') {
      return 'bg-admin-leaf/10 text-admin-leaf';
    }
    if (signal.tone === 'warn') {
      return 'bg-admin-warm/12 text-admin-warm';
    }
    return 'bg-admin-ink/6 text-admin-ink/62';
  }
</script>

<section class="grid gap-4" aria-label="Browser local file access policy">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <p class="admin-eyebrow">Browser policy</p>
      <h2 class="m-0 text-base font-extrabold text-admin-night">{viewModel.title}</h2>
    </div>
    <span class="rounded-full border border-admin-leaf/20 bg-admin-leaf/10 px-3 py-1 text-xs font-extrabold text-admin-leaf" data-testid="policy-mode">
      Mode: {viewModel.mode}
    </span>
  </div>

  <p class="m-0 text-sm leading-normal text-admin-ink/68" data-testid="policy-note">{viewModel.note}</p>

  <div class="grid grid-cols-3 gap-2 max-[760px]:grid-cols-1">
    {#each viewModel.signals as signal}
      <span class={`rounded-[14px] p-3 text-xs font-extrabold uppercase ${signalClass(signal)}`}>
        {signal.label}
        <strong class="mt-1 block text-sm normal-case" data-testid={signal.testId}>{signal.value}</strong>
      </span>
    {/each}
  </div>

  <div class="grid gap-2 rounded-[16px] bg-admin-cream/70 p-3 text-sm text-admin-ink/68">
    <span><strong>Runtime:</strong> {viewModel.runtime}</span>
    <span class="[overflow-wrap:anywhere]" data-testid="policy-cdp-endpoint">
      <strong>CDP endpoint:</strong> {viewModel.cdpEndpoint}
    </span>
  </div>

  <div class="flex flex-wrap gap-2">
    <button class="admin-button-primary" type="button" disabled={!viewModel.canRefresh} onclick={onRefresh}>
      Refresh policy
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="policy-copy-command"
      disabled={!viewModel.canCopyProbeCommand}
      onclick={onCopyProbeCommand}
    >
      {copied ? 'Copied probe command' : 'Copy CDP probe command'}
    </button>
  </div>

  {#if viewModel.probeCommand}
    <code class="block max-h-28 overflow-auto rounded-[14px] bg-admin-night p-3 text-xs text-admin-cream [overflow-wrap:anywhere]" data-testid="policy-probe-command">
      {viewModel.probeCommand}
    </code>
  {/if}
</section>
