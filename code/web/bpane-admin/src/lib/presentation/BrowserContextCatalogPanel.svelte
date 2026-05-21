<script lang="ts">
  import { Clipboard, Database, RefreshCw, Trash2 } from 'lucide-svelte';
  import type { BrowserContextResource, SessionResource } from '../api/control-types';
  import AdminMessage from './AdminMessage.svelte';
  import {
    BrowserContextViewModelBuilder,
    type BrowserContextCatalogRowViewModel,
  } from './browser-context-view-model';

  type BrowserContextCatalogPanelProps = {
    readonly contexts: readonly BrowserContextResource[];
    readonly sessions?: readonly SessionResource[];
    readonly loading?: boolean;
    readonly error?: string | null;
    readonly deletingContextId?: string | null;
    readonly selectedContextId?: string | null;
    readonly onRefresh: () => void;
    readonly onDeleteContext: (contextId: string) => void;
    readonly onSelectContextId?: (contextId: string) => void;
  };

  let {
    contexts,
    sessions = [],
    loading = false,
    error = null,
    deletingContextId = null,
    selectedContextId = undefined,
    onRefresh,
    onDeleteContext,
    onSelectContextId,
  }: BrowserContextCatalogPanelProps = $props();

  let search = $state('');
  let internalSelectedContextId = $state<string | null>(null);
  let copyStatus = $state<string | null>(null);
  const effectiveSelectedContextId = $derived(selectedContextId === undefined ? internalSelectedContextId : selectedContextId);
  const viewModel = $derived(BrowserContextViewModelBuilder.catalog({
    contexts,
    sessions,
    selectedContextId: effectiveSelectedContextId,
    search,
  }));

  $effect(() => {
    const firstRow = viewModel.rows[0] ?? null;
    if (selectedContextId !== undefined) {
      return;
    }
    if (!firstRow) {
      internalSelectedContextId = null;
    } else if (!viewModel.rows.some((row) => row.id === internalSelectedContextId)) {
      internalSelectedContextId = firstRow.id;
    }
  });

  function selectContext(contextId: string): void {
    internalSelectedContextId = contextId;
    onSelectContextId?.(contextId);
    copyStatus = null;
  }

  async function copyApiExample(): Promise<void> {
    if (!viewModel.selectedContext) {
      return;
    }
    try {
      await navigator.clipboard?.writeText(viewModel.apiExample);
      copyStatus = 'API example copied.';
    } catch {
      copyStatus = 'API example is ready to copy from the preview.';
    }
  }

  function deleteSelectedContext(): void {
    const context = viewModel.selectedContext;
    if (!context?.canDelete || deletingContextId === context.id) {
      return;
    }
    onDeleteContext(context.id);
  }

  function stateClass(row: BrowserContextCatalogRowViewModel): string {
    return row.state === 'ready'
      ? 'border-admin-leaf/30 bg-admin-leaf/12 text-admin-leaf'
      : 'border-admin-danger/32 bg-admin-danger/10 text-admin-danger';
  }
</script>

<div class="grid min-w-0 gap-3" data-testid="browser-context-catalog">
  <section class="grid gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Browser context catalog controls">
    <div class="flex min-w-0 flex-wrap items-start justify-between gap-3">
      <div class="min-w-0">
        <p class="admin-eyebrow mb-1">Browser contexts</p>
        <h3 class="m-0 text-base font-bold text-admin-ink">Reusable profile catalog</h3>
      </div>
      <button
        class="admin-button-primary inline-flex items-center gap-2"
        type="button"
        data-testid="browser-context-refresh"
        disabled={loading}
        onclick={onRefresh}
      >
        <RefreshCw size={15} aria-hidden="true" />
        Refresh
      </button>
    </div>

    <div class="grid min-w-0 gap-3 md:grid-cols-[minmax(220px,360px)_1fr] md:items-end">
      <label class="grid min-w-0 gap-1 text-sm font-bold text-admin-ink/72">
        Search
        <input
          class="min-h-11 min-w-0 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="browser-context-search"
          placeholder="Name, label, state, session usage"
          bind:value={search}
        />
      </label>
      <p class="m-0 text-sm text-admin-ink/62" data-testid="browser-context-count">
        {viewModel.rows.length} of {viewModel.totalCount} visible contexts | {viewModel.readyCount} ready | {viewModel.deletedCount} deleted
      </p>
    </div>

    <AdminMessage
      variant="warning"
      title="Credential boundary"
      message={viewModel.secretWarning}
      compact={true}
    />
  </section>

  {#if error}
    <AdminMessage variant="error" message={error} testId="browser-context-error" compact={true} />
  {:else if loading && contexts.length === 0}
    <AdminMessage variant="loading" message="Loading browser contexts..." compact={true} />
  {:else if viewModel.rows.length === 0}
    <AdminMessage
      variant="empty"
      message={viewModel.emptyMessage}
      testId="browser-context-empty"
      compact={true}
    />
  {:else}
    <div class="grid min-w-0 gap-3 xl:grid-cols-[minmax(260px,0.9fr)_minmax(0,1.1fr)]">
      <section class="grid max-h-[min(520px,52vh)] min-w-0 gap-2 overflow-y-auto pr-1" aria-label="Browser context rows">
        {#each viewModel.rows as row}
          <button
            class={`grid min-w-0 cursor-pointer grid-cols-[4px_minmax(0,1fr)] gap-3 rounded-xl border p-3 text-left transition hover:border-admin-leaf/42 hover:bg-admin-field/78 ${
              viewModel.selectedContext?.id === row.id
                ? 'border-admin-leaf/42 bg-admin-field/84'
                : 'border-admin-ink/10 bg-admin-panel/68'
            }`}
            type="button"
            data-testid="browser-context-row"
            data-context-id={row.id}
            aria-pressed={viewModel.selectedContext?.id === row.id}
            onclick={() => selectContext(row.id)}
          >
            <span class={`h-full min-h-14 rounded-full ${viewModel.selectedContext?.id === row.id ? 'bg-admin-leaf' : 'bg-admin-ink/12'}`}></span>
            <span class="grid min-w-0 gap-1">
              <span class="flex min-w-0 items-center gap-2">
                <strong class="truncate text-sm text-admin-ink" title={row.name}>{row.name}</strong>
                <span class={`rounded-full border px-2 py-0.5 text-[0.68rem] font-extrabold ${stateClass(row)}`}>
                  {row.state}
                </span>
              </span>
              <span class="truncate font-mono text-xs text-admin-ink/54">{row.shortId}</span>
              <span class="truncate text-xs text-admin-ink/58">
                {row.persistence} | {row.sessionSummary} | {row.labels}
              </span>
            </span>
          </button>
        {/each}
      </section>

      <section class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-panel/62 p-3" aria-label="Browser context detail">
        {#if viewModel.selectedContext}
          {@const context = viewModel.selectedContext}
          <div class="flex min-w-0 flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <p class="admin-eyebrow mb-1">Selected context</p>
              <h3 class="m-0 truncate text-base font-bold text-admin-ink" data-testid="browser-context-detail-name" title={context.name}>
                {context.name}
              </h3>
              <p class="m-0 mt-1 truncate font-mono text-xs text-admin-ink/54">{context.id}</p>
            </div>
            <span class={`w-fit rounded-xl border px-3 py-1 text-xs font-bold ${stateClass(context)}`} data-testid="browser-context-detail-state">
              {context.state}
            </span>
          </div>

          <p class="m-0 text-sm leading-normal text-admin-ink/66">{context.description}</p>

          <div class="grid min-w-0 grid-cols-2 gap-2 text-xs text-admin-ink/70">
            {@render Fact('Persistence', context.persistence, 'browser-context-detail-persistence')}
            {@render Fact('References', context.sessionSummary, 'browser-context-detail-references')}
            {@render Fact('Active writer', context.activeRuntimeSummary, 'browser-context-detail-active-writer')}
            {@render Fact('Profile storage', context.profileStorageSummary, 'browser-context-detail-storage')}
            {@render Fact('Storage limit', context.profileStorageLimitSummary, 'browser-context-detail-storage-limit')}
            {@render Fact('Retention', context.retentionSummary, 'browser-context-detail-retention')}
            {@render Fact('Last used', context.lastUsedAt, 'browser-context-detail-last-used')}
            {@render Fact('Updated', context.updatedAt, 'browser-context-detail-updated')}
            {@render Fact('Created', context.createdAt, 'browser-context-detail-created')}
            {@render Fact('Deleted', context.deletedAt, 'browser-context-detail-deleted')}
          </div>

          <AdminMessage
            variant={context.canDelete ? 'info' : 'warning'}
            message={context.deleteHint}
            testId="browser-context-delete-hint"
            compact={true}
          />

          <div class="flex flex-wrap gap-2">
            <button
              class="admin-button-ghost inline-flex items-center gap-2"
              type="button"
              data-testid="browser-context-copy-api"
              onclick={() => void copyApiExample()}
            >
              <Clipboard size={15} aria-hidden="true" />
              Copy API example
            </button>
            <button
              class="admin-button-ghost inline-flex items-center gap-2 border-admin-danger/30 text-admin-danger"
              type="button"
              data-testid="browser-context-delete"
              disabled={!context.canDelete || deletingContextId === context.id || loading}
              onclick={deleteSelectedContext}
            >
              {#if deletingContextId === context.id}
                <RefreshCw class="animate-spin" size={15} aria-hidden="true" />
                Deleting
              {:else}
                <Trash2 size={15} aria-hidden="true" />
                Delete
              {/if}
            </button>
          </div>

          {#if copyStatus}
            <AdminMessage variant="success" message={copyStatus} testId="browser-context-copy-message" compact={true} />
          {/if}

          <section class="rounded-xl border border-admin-ink/10 bg-admin-field/68 p-3" aria-label="Browser context API examples">
            <div class="mb-2 flex items-center gap-2 text-xs font-bold uppercase text-[#c1d0e8]">
              <Database size={14} aria-hidden="true" />
              API examples
            </div>
            <pre class="m-0 max-h-52 overflow-auto whitespace-pre-wrap text-xs text-admin-ink" data-testid="browser-context-api-example">{viewModel.apiExample}</pre>
          </section>
        {/if}
      </section>
    </div>
  {/if}
</div>

{#snippet Fact(label: string, value: string, testId: string)}
  <span class="min-w-0 rounded-xl bg-admin-field/72 p-2 font-bold uppercase">
    {label}
    <strong class="mt-1 block truncate font-mono text-admin-ink normal-case" data-testid={testId} title={value}>{value}</strong>
  </span>
{/snippet}
