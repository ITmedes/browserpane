<script lang="ts">
  import type { McpDelegationTone, McpDelegationViewModel } from './mcp-delegation-view-model';

  type McpDelegationPanelProps = {
    readonly viewModel: McpDelegationViewModel;
    readonly onRefresh: () => void;
    readonly onDelegate: () => void;
    readonly onClear: () => void;
  };

  let { viewModel, onRefresh, onDelegate, onClear }: McpDelegationPanelProps = $props();

  function toneClass(tone: McpDelegationTone): string {
    if (tone === 'active') {
      return 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf';
    }
    if (tone === 'warning') {
      return 'border-admin-warm/30 bg-admin-warm/12 text-admin-warm';
    }
    if (tone === 'unavailable') {
      return 'border-admin-danger/30 bg-admin-danger/10 text-admin-danger';
    }
    return 'border-admin-ink/12 bg-admin-cream/72 text-admin-ink/70';
  }
</script>

<section class="mt-4 grid gap-3 rounded-[18px] border border-admin-ink/10 bg-admin-cream/58 p-3" aria-label="MCP delegation">
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div>
      <p class="admin-eyebrow mb-1">MCP delegation</p>
      <h3 class="m-0 text-sm font-extrabold text-admin-night">{viewModel.title}</h3>
    </div>
    <span
      class={`rounded-full border px-3 py-1 text-xs font-extrabold ${toneClass(viewModel.tone)}`}
      data-testid="mcp-status"
    >
      {viewModel.status}
    </span>
  </div>

  <p class="m-0 text-sm leading-normal text-admin-ink/68" data-testid="mcp-note">{viewModel.note}</p>

  <div class="flex flex-wrap gap-2">
    <button
      class="admin-button-primary"
      type="button"
      data-testid="mcp-refresh"
      disabled={!viewModel.canRefresh}
      onclick={onRefresh}
    >
      Refresh MCP
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="mcp-delegate"
      disabled={!viewModel.canDelegate}
      onclick={onDelegate}
    >
      Delegate MCP
    </button>
    <button
      class="admin-button-primary"
      type="button"
      data-testid="mcp-clear"
      disabled={!viewModel.canClear}
      onclick={onClear}
    >
      Clear bridge
    </button>
  </div>

  {#if viewModel.busy}
    <p class="admin-empty mt-0" data-testid="mcp-busy">Updating MCP delegation...</p>
  {/if}
  {#if viewModel.error}
    <p class="admin-error mt-0" data-testid="mcp-error">{viewModel.error}</p>
  {/if}
</section>
