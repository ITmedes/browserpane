<script lang="ts">
  import type { McpDelegationTone, McpDelegationViewModel } from './mcp-delegation-view-model';

  type McpDelegationPanelProps = {
    readonly viewModel: McpDelegationViewModel;
    readonly onRefresh: () => void;
    readonly onAuthorize: () => void;
    readonly onRevoke: () => void;
    readonly onSetDefault: () => void;
    readonly onClearDefault: () => void;
    readonly onCopyEndpoint: () => void;
  };

  let {
    viewModel,
    onRefresh,
    onAuthorize,
    onRevoke,
    onSetDefault,
    onClearDefault,
    onCopyEndpoint,
  }: McpDelegationPanelProps = $props();

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

<section class="mt-3 grid gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="MCP delegation">
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="min-w-0">
      <p class="admin-eyebrow mb-1">Selected session MCP</p>
      <h3 class="m-0 truncate text-sm font-extrabold text-admin-ink">{viewModel.title}</h3>
    </div>
    <span
      class={`rounded-full border px-3 py-1 text-xs font-extrabold ${toneClass(viewModel.tone)}`}
      data-testid="mcp-status"
    >
      {viewModel.status}
    </span>
  </div>

  <p class="m-0 text-sm leading-normal text-admin-ink/68" data-testid="mcp-note">{viewModel.note}</p>

  {#if viewModel.endpointUrl}
    <div class="flex flex-col gap-2 rounded-xl border border-admin-ink/10 bg-admin-night/35 p-3 md:flex-row md:items-center">
      <div class="min-w-0 flex-1">
        <p class="admin-eyebrow mb-1">Session MCP endpoint</p>
        <code class="block break-all text-xs font-bold text-admin-ink/78" data-testid="mcp-endpoint-url">{viewModel.endpointUrl}</code>
      </div>
      <button
        class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
        type="button"
        data-testid="mcp-copy-endpoint"
        disabled={!viewModel.canCopyEndpoint}
        onclick={onCopyEndpoint}
      >
        Copy URL
      </button>
    </div>
  {/if}

  {#if viewModel.healthSummary}
    <p class="m-0 text-xs font-bold text-admin-ink/62" data-testid="mcp-health-summary">{viewModel.healthSummary}</p>
  {/if}

  <div class="flex flex-wrap gap-2">
    <button
      class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="mcp-refresh"
      disabled={!viewModel.canRefresh}
      onclick={onRefresh}
    >
      Refresh MCP
    </button>
    <button
      class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="mcp-delegate"
      disabled={!viewModel.canAuthorize}
      onclick={onAuthorize}
    >
      Authorize MCP
    </button>
    <button
      class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="mcp-revoke"
      disabled={!viewModel.canRevoke}
      onclick={onRevoke}
    >
      Revoke
    </button>
    <button
      class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="mcp-set-default"
      disabled={!viewModel.canSetDefault}
      onclick={onSetDefault}
    >
      Set default
    </button>
    <button
      class="rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 py-2 text-xs font-bold text-admin-ink disabled:cursor-not-allowed disabled:opacity-45"
      type="button"
      data-testid="mcp-clear"
      disabled={!viewModel.canClearDefault}
      onclick={onClearDefault}
    >
      Clear default
    </button>
  </div>

  {#if viewModel.busy}
    <p class="admin-empty mt-0" data-testid="mcp-busy">Updating MCP delegation...</p>
  {/if}
  {#if viewModel.error}
    <p class="admin-error mt-0" data-testid="mcp-error">{viewModel.error}</p>
  {/if}
</section>
