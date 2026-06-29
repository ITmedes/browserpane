<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    browserContextMatchesSearch,
    buildBrowserContextOverviewModel,
    type BrowserContextOverviewRow,
  } from '$lib/browser-contexts/browser-context-overview-view-model';
  import type {
    BrowserContextPersistenceMode,
    BrowserContextResource,
    BrowserContextState,
  } from '$lib/browser-contexts/browser-context-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type BrowserContextLens = 'all' | BrowserContextState | BrowserContextPersistenceMode | 'active' | 'attention';

  type BrowserContextLensDefinition = {
    readonly id: BrowserContextLens;
    readonly label: string;
    readonly count: number;
    readonly showDot?: boolean;
  };

  type BrowserContextCatalogTableProps = {
    readonly contexts: readonly BrowserContextResource[];
  };

  let { contexts }: BrowserContextCatalogTableProps = $props();
  let contextLens = $state<BrowserContextLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildBrowserContextOverviewModel(contexts));
  const lensDefinitions = $derived(buildLensDefinitions(model.rows));
  const visibleRows = $derived(filterRows(model.rows, contextLens, searchQuery));

  function buildLensDefinitions(rows: readonly BrowserContextOverviewRow[]): readonly BrowserContextLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      {
        id: 'ready',
        label: 'Ready',
        count: rows.filter((row) => row.state === 'ready').length,
        showDot: true,
      },
      { id: 'deleted', label: 'Deleted', count: rows.filter((row) => row.state === 'deleted').length },
      { id: 'reusable', label: 'Reusable', count: rows.filter((row) => row.persistence === 'reusable').length },
      { id: 'ephemeral', label: 'Ephemeral', count: rows.filter((row) => row.persistence === 'ephemeral').length },
      { id: 'active', label: 'Active use', count: rows.filter((row) => row.usageTone === 'warning').length },
      { id: 'attention', label: 'Needs attention', count: rows.filter((row) => row.state === 'deleted' || row.storageTone === 'danger').length },
    ];
  }

  function filterRows(
    rows: readonly BrowserContextOverviewRow[],
    lens: BrowserContextLens,
    query: string,
  ): readonly BrowserContextOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, lens) && browserContextMatchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: BrowserContextOverviewRow, lens: BrowserContextLens): boolean {
    if (lens === 'ready' || lens === 'deleted') {
      return row.state === lens;
    }
    if (lens === 'reusable' || lens === 'ephemeral') {
      return row.persistence === lens;
    }
    if (lens === 'active') {
      return row.usageTone === 'warning';
    }
    if (lens === 'attention') {
      return row.state === 'deleted' || row.storageTone === 'danger';
    }
    return true;
  }

  function detailHref(contextId: string): string {
    return `/admin-new/browser-contexts/${encodeURIComponent(contextId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="browser-contexts-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Browser context catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Reusable profiles, project scope, retention, storage, and active-use posture.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="browser-contexts-list-count">
      {visibleRows.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            contextLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={contextLens === lens.id}
          onclick={() => {
            contextLens = lens.id;
          }}
          data-testid={`browser-contexts-lens-${lens.id}`}
        >
          {#if lens.showDot}
            <span class="h-1.5 w-1.5 rounded-full bg-admin-success" aria-hidden="true"></span>
          {/if}
          <span>{lens.label}</span>
          <span class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted">
            {lens.count}
          </span>
        </button>
      {/each}
    </div>

    <label class="mx-4 mb-3 flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[360px]">
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Search browser contexts</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Context, state, scope, storage..."
        bind:value={searchQuery}
        data-testid="browser-contexts-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[1120px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Context</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Persistence</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Scope</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Retention</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Storage</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Usage</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Last used</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Updated</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="10" data-testid="browser-contexts-filter-empty">
              No browser contexts match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleRows as row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="browser-contexts-list-row">
              <td class="w-[270px] px-4 py-3 align-middle">
                <div class="grid min-w-0 text-left">
                  <span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span>
                  <span class="mt-1 truncate text-xs text-admin-muted">{row.description}</span>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                </div>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>
                  {row.state}
                </span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.persistenceTone)}`}>
                  {row.persistenceLabel}
                </span>
              </td>
              <td class="max-w-[170px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.scope}</span>
              </td>
              <td class="max-w-[210px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.retentionSummary}</span>
              </td>
              <td class="max-w-[210px] px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.storageTone)}`}>
                  {row.storageSummary}
                </span>
              </td>
              <td class="max-w-[190px] px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.usageTone)}`}>
                  {row.usageSummary}
                </span>
              </td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.lastUsedAt}</td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.updatedAt}</td>
              <td class="px-4 py-3 align-middle text-right">
                <a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={detailHref(row.id)}
                  data-testid="browser-contexts-detail-link"
                >
                  Details
                </a>
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</section>
