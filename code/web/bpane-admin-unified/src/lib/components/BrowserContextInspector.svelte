<script lang="ts">
  import { Copy, RefreshCw, Trash2 } from '@lucide/svelte';
  import type {
    BrowserContextActionState,
    BrowserContextDetailLoadState,
  } from '$lib/browser-contexts/browser-context-detail-state';
  import {
    buildBrowserContextStatusSummaryModel,
  } from '$lib/browser-contexts/browser-context-edit-view-model';
  import {
    browserContextRow,
    retentionSummary,
    storageSummary,
  } from '$lib/browser-contexts/browser-context-overview-view-model';
  import type { BrowserContextResource } from '$lib/browser-contexts/browser-context-types';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import BrowserContextStatusSummary from './BrowserContextStatusSummary.svelte';

  type BrowserContextInspectorProps = {
    readonly state: BrowserContextDetailLoadState;
    readonly actionState?: BrowserContextActionState;
    readonly onRefreshContext?: () => void | Promise<void>;
    readonly onDeleteContext?: () => void | Promise<void>;
  };

  let {
    state,
    actionState = { status: 'idle' },
    onRefreshContext,
    onDeleteContext,
  }: BrowserContextInspectorProps = $props();

  const row = $derived(state.status === 'ready' ? browserContextRow(state.context) : null);
  const statusSummary = $derived(state.status === 'ready' ? buildBrowserContextStatusSummaryModel(state.context) : null);
  const busy = $derived(actionState.status === 'running');
  const deleteBlockedReason = $derived(state.status === 'ready' ? browserContextDeleteBlockedReason(state.context) : null);

  function refreshContext(): void {
    void onRefreshContext?.();
  }

  function deleteContext(): void {
    void onDeleteContext?.();
  }

  async function copyContextId(contextId: string): Promise<void> {
    await navigator.clipboard?.writeText(contextId);
  }

  function browserContextDeleteBlockedReason(context: BrowserContextResource): string | null {
    if (context.state === 'deleted') {
      return 'Already deleted';
    }
    if (context.usage.active_runtime_session_count > 0) {
      return 'Active runtime sessions must stop first';
    }
    if (context.usage.visible_session_count > 0) {
      return 'Visible session references must be cleared first';
    }
    return null;
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="browser-context-inspector">
  {#if state.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-center text-sm text-admin-muted" data-testid="browser-context-inspector-idle">
      Select a browser context to inspect.
    </div>
  {:else if state.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="browser-context-inspector-loading">
      Loading browser context...
    </div>
  {:else if state.status === 'error'}
    <div class="p-4">
      <AdminMessage
        tone="error"
        title="Browser context unavailable"
        message={state.message}
        testId="browser-context-inspector-error"
      />
    </div>
  {:else if row && statusSummary}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 min-w-0 max-w-full truncate text-xl font-semibold text-admin-ink" data-testid="browser-context-detail-name">{row.name}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`} data-testid="browser-context-detail-state">
              {row.state}
            </span>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.persistenceTone)}`}>
              {row.persistenceLabel}
            </span>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.storageTone)}`}>
              {row.storageSummary}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{row.description}</p>
          <p class="m-0 mt-2 min-w-0 truncate font-mono text-xs text-admin-muted">{row.id}</p>
        </div>

        <div class="flex shrink-0 flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            type="button"
            onclick={() => void copyContextId(row.id)}
            data-testid="browser-context-copy-id"
          >
            <Copy size={15} strokeWidth={1.8} />
            <span>Copy ID</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={refreshContext}
            disabled={busy}
            data-testid="browser-context-refresh-detail"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Refresh</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-rose-200 bg-rose-50 px-3 text-sm font-medium text-rose-800 hover:bg-rose-100 disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={deleteContext}
            disabled={busy || Boolean(deleteBlockedReason)}
            title={deleteBlockedReason ?? 'Soft-delete browser context'}
            data-testid="browser-context-delete"
          >
            <Trash2 size={15} strokeWidth={1.8} />
            <span>Delete</span>
          </button>
        </div>
      </div>
    </div>

    {#if actionState.status === 'success'}
      <div class="border-b border-admin-border p-4">
        <AdminMessage tone="success" title="Browser context action completed" message={actionState.message} testId="browser-context-action-success" />
      </div>
    {:else if actionState.status === 'error'}
      <div class="border-b border-admin-border p-4">
        <AdminMessage tone="error" title="Browser context action failed" message={actionState.message} testId="browser-context-action-error" />
      </div>
    {:else if actionState.status === 'running'}
      <div class="border-b border-admin-border p-4">
        <AdminMessage tone="loading" title={actionState.label} testId="browser-context-action-running" />
      </div>
    {/if}

    <div class="grid gap-4 p-4">
      <BrowserContextStatusSummary model={statusSummary} />

      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="browser-context-detail-configuration">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Configuration</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
            Existing context metadata is read-only in the current control API. Create a new context when scope or limits need to change.
          </p>
        </div>

        <dl class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Scope</dt>
            <dd class="m-0 mt-1 truncate text-sm font-medium text-admin-ink">{row.scope}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="browser-context-detail-retention">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Retention</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">{retentionSummary(state.context)}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="browser-context-detail-storage-limit">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Profile storage</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">{storageSummary(state.context)}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="browser-context-detail-references">
            <dt class="text-xs font-semibold uppercase text-admin-muted">References</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">{state.context.usage.visible_session_count} visible sessions</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Runtime</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">
              {state.context.usage.active_runtime_session_count} active runtime
              {#if state.context.usage.active_runtime_session_id}
                <span class="block truncate font-mono text-xs text-admin-muted">{state.context.usage.active_runtime_session_id}</span>
              {/if}
            </dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Last used</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">{state.context.last_used_at ? formatDateTime(state.context.last_used_at) : 'Never'}</dd>
          </div>
        </dl>
      </section>

      <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="browser-context-detail-labels">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Labels</h3>
        </div>

        {#if Object.entries(state.context.labels).length === 0}
          <p class="m-0 mt-4 rounded-md border border-dashed border-admin-border bg-admin-panel p-4 text-sm text-admin-muted">
            No labels configured.
          </p>
        {:else}
          <dl class="mt-4 grid gap-2 md:grid-cols-2">
            {#each Object.entries(state.context.labels) as [key, value]}
              <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
                <dt class="truncate font-mono text-xs font-semibold text-admin-muted">{key}</dt>
                <dd class="m-0 mt-1 truncate text-sm font-medium text-admin-ink">{value}</dd>
              </div>
            {/each}
          </dl>
        {/if}
      </section>
    </div>
  {/if}
</aside>
