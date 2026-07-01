<script lang="ts">
  import { ExternalLink, RefreshCw, Square, SquareX, Unplug, XCircle } from '@lucide/svelte';
  import {
    buildSessionDetailModel,
    type SessionDetailLoadState,
    type SessionFact,
  } from '$lib/sessions/session-detail-view-model';
  import type { SessionActionState } from '$lib/sessions/session-overview-view-model';
  import type { McpDelegationViewModel } from '$lib/mcp/mcp-delegation-view-model';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import SessionMcpDelegationCard from './SessionMcpDelegationCard.svelte';

  type SessionInspectorProps = {
    readonly state: SessionDetailLoadState;
    readonly actionState?: SessionActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onConnectPreview?: (sessionId: string) => void | Promise<void>;
    readonly onCancelQueue?: () => void | Promise<void>;
    readonly onDisconnectAll?: () => void | Promise<void>;
    readonly onRelease?: () => void | Promise<void>;
    readonly onStop?: () => void | Promise<void>;
    readonly onKill?: () => void | Promise<void>;
    readonly mcpViewModel?: McpDelegationViewModel | null;
    readonly mcpActionState?: SessionActionState;
    readonly onMcpRefresh?: () => void | Promise<void>;
    readonly onMcpAuthorize?: () => void | Promise<void>;
    readonly onMcpRevoke?: () => void | Promise<void>;
    readonly onMcpSetDefault?: () => void | Promise<void>;
    readonly onMcpClearDefault?: () => void | Promise<void>;
    readonly onMcpCopyEndpoint?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    actionState = { status: 'idle' },
    onRefresh,
    onConnectPreview,
    onCancelQueue,
    onDisconnectAll,
    onRelease,
    onStop,
    onKill,
    mcpViewModel = null,
    mcpActionState = { status: 'idle' },
    onMcpRefresh,
    onMcpAuthorize,
    onMcpRevoke,
    onMcpSetDefault,
    onMcpClearDefault,
    onMcpCopyEndpoint,
  }: SessionInspectorProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildSessionDetailModel(loadState.session, loadState.liveStatus)
    : null);
  const busy = $derived(actionState.status === 'running' || loadState.status === 'loading');

  function refresh(): void {
    void onRefresh?.();
  }

  function factTextClass(fact: SessionFact): string {
    if (fact.tone === 'success') {
      return 'text-emerald-700';
    }
    if (fact.tone === 'warning') {
      return 'text-amber-700';
    }
    if (fact.tone === 'danger') {
      return 'text-red-700';
    }
    if (fact.tone === 'neutral') {
      return 'text-slate-600';
    }
    return 'text-admin-ink';
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="session-inspector">
  {#if loadState.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-center text-sm text-admin-muted" data-testid="session-inspector-idle">
      Select a session to inspect.
    </div>
  {:else if loadState.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="session-inspector-loading">
      Loading session details...
    </div>
  {:else if loadState.status === 'error'}
    <div class="p-4">
      <AdminMessage
        tone="error"
        title="Session unavailable"
        message={loadState.message}
        testId="session-inspector-error"
      />
    </div>
  {:else if model}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 min-w-0 max-w-full truncate font-mono text-xl font-semibold text-admin-ink" data-testid="session-detail-title">{model.shortId}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.stateTone)}`} data-testid="session-detail-state">
              {model.state}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{model.subtitle}</p>
          <p class="m-0 mt-2 min-w-0 truncate font-mono text-xs text-admin-muted">{model.id}</p>
        </div>

        <button
          class="inline-flex h-9 shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={refresh}
          disabled={busy}
          data-testid="session-detail-refresh"
        >
          <RefreshCw size={15} strokeWidth={1.8} />
          <span>Refresh</span>
        </button>
      </div>
    </div>

    {#if actionState.status === 'success'}
      <div class="border-b border-admin-border p-4">
        <AdminMessage tone="success" title="Session action completed" message={actionState.message} testId="session-detail-action-success" />
      </div>
    {:else if actionState.status === 'error'}
      <div class="border-b border-admin-border p-4">
        <AdminMessage tone="error" title="Session action failed" message={actionState.message} testId="session-detail-action-error" />
      </div>
    {:else if actionState.status === 'running'}
      <div class="border-b border-admin-border p-4">
        <AdminMessage tone="loading" title={actionState.label} testId="session-detail-action-running" />
      </div>
    {/if}

    <div class="grid gap-4 p-4">
      <section class="flex flex-wrap gap-2 rounded-md border border-admin-border bg-admin-soft/50 p-4" aria-label="Session lifecycle actions">
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void onConnectPreview?.(model.id)}
          disabled={busy || !model.actions.canConnectPreview}
          title={model.actions.connectPreviewDescription}
          aria-label={model.actions.connectPreviewDescription}
          data-testid="session-connect-preview"
        >
          <ExternalLink size={15} strokeWidth={1.8} />
          <span>{model.actions.connectPreviewLabel}</span>
        </button>
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void onDisconnectAll?.()}
          disabled={busy || !model.actions.canDisconnectAll}
          data-testid="session-disconnect-all"
        >
          <Unplug size={15} strokeWidth={1.8} />
          <span>Disconnect all</span>
        </button>
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void onCancelQueue?.()}
          disabled={busy || !model.actions.canCancelQueue}
          data-testid="session-cancel-queue"
        >
          <XCircle size={15} strokeWidth={1.8} />
          <span>Cancel queued</span>
        </button>
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void onRelease?.()}
          disabled={busy || !model.actions.canRelease}
          data-testid="session-release-runtime"
        >
          <Square size={15} strokeWidth={1.8} />
          <span>Release runtime</span>
        </button>
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void onStop?.()}
          disabled={busy || !model.actions.canStop}
          data-testid="session-stop"
        >
          <Square size={15} strokeWidth={1.8} />
          <span>Stop</span>
        </button>
        <button
          class="inline-flex h-9 items-center gap-2 rounded-md border border-red-200 bg-red-50 px-3 text-sm font-medium text-red-700 hover:bg-red-100 disabled:cursor-not-allowed disabled:opacity-60"
          type="button"
          onclick={() => void onKill?.()}
          disabled={busy || !model.actions.canKill}
          data-testid="session-kill"
        >
          <SquareX size={15} strokeWidth={1.8} />
          <span>Kill</span>
        </button>
      </section>

      {#if mcpViewModel}
        <SessionMcpDelegationCard
          viewModel={mcpViewModel}
          actionState={mcpActionState}
          onRefresh={onMcpRefresh}
          onAuthorize={onMcpAuthorize}
          onRevoke={onMcpRevoke}
          onSetDefault={onMcpSetDefault}
          onClearDefault={onMcpClearDefault}
          onCopyEndpoint={onMcpCopyEndpoint}
        />
      {/if}

      {#each model.sections as section}
        <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid={section.testId}>
          <div class="border-b border-admin-border pb-3">
            <h3 class="m-0 text-sm font-semibold text-admin-ink">{section.title}</h3>
          </div>
          <dl class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            {#each section.facts as fact}
              <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
                <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
                <dd class={`m-0 mt-1 min-w-0 break-words text-sm font-medium ${factTextClass(fact)}`} data-testid={fact.testId}>
                  {fact.value}
                </dd>
              </div>
            {/each}
          </dl>
        </section>
      {/each}

      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="session-detail-live-connections">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Live Connections</h3>
        </div>
        {#if model.connections.length === 0}
          <p class="m-0 mt-4 rounded-md border border-dashed border-admin-border bg-admin-panel p-4 text-sm text-admin-muted" data-testid="session-connections-empty">
            No live connections reported.
          </p>
        {:else}
          <div class="mt-4 grid gap-2">
            {#each model.connections as connection}
              <div class="flex min-w-0 items-center justify-between gap-3 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="session-connection-row">
                <span class="min-w-0">
                  <strong class="block font-mono text-sm text-admin-ink">#{connection.id}</strong>
                  <span class="text-xs uppercase text-admin-muted">{connection.role}</span>
                </span>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {/if}
</aside>
