<script lang="ts">
  import { Copy, RefreshCw, ShieldCheck, ShieldOff, Star, XCircle } from '@lucide/svelte';
  import type { McpDelegationTone, McpDelegationViewModel } from '$lib/mcp/mcp-delegation-view-model';
  import type { SessionActionState } from '$lib/sessions/session-overview-view-model';
  import ActionFeedback from './ActionFeedback.svelte';

  type SessionMcpDelegationCardProps = {
    readonly viewModel: McpDelegationViewModel;
    readonly actionState?: SessionActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onAuthorize?: () => void | Promise<void>;
    readonly onRevoke?: () => void | Promise<void>;
    readonly onSetDefault?: () => void | Promise<void>;
    readonly onClearDefault?: () => void | Promise<void>;
    readonly onCopyEndpoint?: () => void | Promise<void>;
  };

  let {
    viewModel,
    actionState = { status: 'idle' },
    onRefresh,
    onAuthorize,
    onRevoke,
    onSetDefault,
    onClearDefault,
    onCopyEndpoint,
  }: SessionMcpDelegationCardProps = $props();

  function statusClass(tone: McpDelegationTone): string {
    if (tone === 'success') {
      return 'bg-emerald-50 text-emerald-700 ring-emerald-200';
    }
    if (tone === 'warning') {
      return 'bg-amber-50 text-amber-700 ring-amber-200';
    }
    if (tone === 'danger') {
      return 'bg-red-50 text-red-700 ring-red-200';
    }
    return 'bg-slate-100 text-slate-700 ring-slate-200';
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-mcp-delegation">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-3 lg:flex-row lg:items-start lg:justify-between" data-testid="mcp-summary-row">
    <div class="min-w-0">
      <div class="flex min-w-0 flex-wrap items-center gap-2">
        <h3 class="m-0 text-sm font-semibold text-admin-ink">MCP Delegation</h3>
        <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${statusClass(viewModel.statusTone)}`} data-testid="mcp-delegation-status">
          {viewModel.statusLabel}
        </span>
      </div>
      <p class="m-0 mt-1 text-sm text-admin-muted" data-testid="mcp-delegation-description">{viewModel.description}</p>
    </div>

    <button
      class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void onRefresh?.()}
      disabled={!viewModel.canRefresh}
      data-testid="mcp-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh</span>
    </button>
  </div>

  <div class="mt-4 flex w-full flex-col gap-3">
    <div class="flex w-full flex-wrap gap-2" data-testid="mcp-actions-row">
      <button
        class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void onAuthorize?.()}
        disabled={!viewModel.canAuthorize}
        data-testid="mcp-authorize"
      >
        <ShieldCheck size={15} strokeWidth={1.8} />
        <span>Authorize</span>
      </button>
      <button
        class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void onSetDefault?.()}
        disabled={!viewModel.canSetDefault}
        data-testid="mcp-set-default"
      >
        <Star size={15} strokeWidth={1.8} />
        <span>Set default</span>
      </button>
      <button
        class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void onClearDefault?.()}
        disabled={!viewModel.canClearDefault}
        data-testid="mcp-clear-default"
      >
        <XCircle size={15} strokeWidth={1.8} />
        <span>Clear default</span>
      </button>
      <button
        class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void onRevoke?.()}
        disabled={!viewModel.canRevoke}
        data-testid="mcp-revoke"
      >
        <ShieldOff size={15} strokeWidth={1.8} />
        <span>Revoke</span>
      </button>
      <button
        class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={() => void onCopyEndpoint?.()}
        disabled={!viewModel.canCopyEndpoint}
        data-testid="mcp-copy-endpoint"
      >
        <Copy size={15} strokeWidth={1.8} />
        <span>Copy endpoint</span>
      </button>
    </div>

    <div class="w-full min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="mcp-endpoint-row">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Session endpoint</p>
      {#if viewModel.endpointUrl}
        <code class="mt-1 block min-w-0 break-all text-sm font-medium text-admin-ink" data-testid="mcp-endpoint-url">{viewModel.endpointUrl}</code>
      {:else}
        <p class="m-0 mt-1 text-sm text-admin-muted" data-testid="mcp-endpoint-url">Unavailable</p>
      {/if}
    </div>
  </div>

  <dl class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
    <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
      <dt class="text-xs font-semibold uppercase text-admin-muted">Default session</dt>
      <dd class="m-0 mt-1 min-w-0 break-words text-sm font-medium text-admin-ink" data-testid="mcp-default-session">
        {viewModel.defaultSessionLabel}
      </dd>
    </div>
    <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
      <dt class="text-xs font-semibold uppercase text-admin-muted">Clients</dt>
      <dd class="m-0 mt-1 min-w-0 break-words text-sm font-medium text-admin-ink" data-testid="mcp-client-summary">
        {viewModel.clientSummary}
      </dd>
    </div>
    <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
      <dt class="text-xs font-semibold uppercase text-admin-muted">Ownership</dt>
      <dd class="m-0 mt-1 min-w-0 break-words text-sm font-medium text-admin-ink" data-testid="mcp-ownership">
        {viewModel.ownershipLabel}
      </dd>
    </div>
    <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
      <dt class="text-xs font-semibold uppercase text-admin-muted">Bridge</dt>
      <dd class="m-0 mt-1 min-w-0 break-words text-sm font-medium text-admin-ink" data-testid="mcp-bridge-summary">
        {viewModel.bridgeSummary}
      </dd>
    </div>
  </dl>

  <div class="mt-4">
    <ActionFeedback
      state={actionState}
      successTitle="MCP action completed"
      errorTitle="MCP action failed"
      successTestId="mcp-action-success"
      errorTestId="mcp-action-error"
      runningTestId="mcp-action-running"
    />
  </div>
</section>
