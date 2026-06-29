<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildWorkflowOverviewModel,
    workflowMatchesSearch,
    type WorkflowOverviewRow,
    type WorkflowVersionMap,
  } from '$lib/workflows/workflow-overview-view-model';
  import type { WorkflowDefinitionResource } from '$lib/workflows/workflow-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type WorkflowLens = 'all' | 'templates' | 'workflows' | 'published' | 'unpublished' | 'attention';

  type WorkflowLensDefinition = {
    readonly id: WorkflowLens;
    readonly label: string;
    readonly count: number;
  };

  type WorkflowCatalogTableProps = {
    readonly definitions: readonly WorkflowDefinitionResource[];
    readonly versions?: WorkflowVersionMap;
    readonly hiddenCount?: number;
  };

  let {
    definitions,
    versions = {},
    hiddenCount = 0,
  }: WorkflowCatalogTableProps = $props();
  let workflowLens = $state<WorkflowLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildWorkflowOverviewModel(definitions, versions, hiddenCount));
  const lensDefinitions = $derived(buildLensDefinitions(model.rows));
  const visibleRows = $derived(filterRows(model.rows, workflowLens, searchQuery));

  function buildLensDefinitions(rows: readonly WorkflowOverviewRow[]): readonly WorkflowLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      { id: 'templates', label: 'Templates', count: rows.filter((row) => row.kind !== 'Workflow').length },
      { id: 'workflows', label: 'Workflows', count: rows.filter((row) => row.kind === 'Workflow').length },
      { id: 'published', label: 'Published', count: rows.filter((row) => row.latestVersion !== 'No version').length },
      { id: 'unpublished', label: 'No version', count: rows.filter((row) => row.latestVersion === 'No version').length },
      {
        id: 'attention',
        label: 'Needs attention',
        count: rows.filter((row) => row.executor === 'Version metadata unavailable').length,
      },
    ];
  }

  function filterRows(
    rows: readonly WorkflowOverviewRow[],
    lens: WorkflowLens,
    query: string,
  ): readonly WorkflowOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, lens) && workflowMatchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: WorkflowOverviewRow, lens: WorkflowLens): boolean {
    if (lens === 'templates') {
      return row.kind !== 'Workflow';
    }
    if (lens === 'workflows') {
      return row.kind === 'Workflow';
    }
    if (lens === 'published') {
      return row.latestVersion !== 'No version';
    }
    if (lens === 'unpublished') {
      return row.latestVersion === 'No version';
    }
    if (lens === 'attention') {
      return row.executor === 'Version metadata unavailable';
    }
    return true;
  }

  function detailHref(workflowId: string): string {
    return `/admin-new/workflows/${encodeURIComponent(workflowId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="workflows-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Workflow catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Definition templates, source metadata, and safe execution references.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="workflows-list-count">
      {visibleRows.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            workflowLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={workflowLens === lens.id}
          onclick={() => {
            workflowLens = lens.id;
          }}
          data-testid={`workflows-lens-${lens.id}`}
        >
          <span>{lens.label}</span>
          <span class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted">
            {lens.count}
          </span>
        </button>
      {/each}
    </div>

    <label class="mx-4 mb-3 flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[360px]">
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Search workflows</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Name, source, executor, label..."
        bind:value={searchQuery}
        data-testid="workflows-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[980px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Workflow</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Kind</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Version</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Executor</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Source</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="6" data-testid="workflows-filter-empty">
              No workflows match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleRows as row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="workflows-list-row">
              <td class="w-[320px] px-4 py-3 align-middle">
                <div class="grid min-w-0 text-left">
                  <span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span>
                  <span class="mt-1 line-clamp-2 text-xs text-admin-muted">{row.description}</span>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                </div>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.kindTone)}`}>
                  {row.kind}
                </span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.latestTone)}`}>
                  {row.latestVersion}
                </span>
              </td>
              <td class="max-w-[180px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="truncate">{row.executor}</span>
              </td>
              <td class="max-w-[260px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.source}</span>
                <span class="mt-1 block truncate font-mono text-[11px]">{row.sourceCommit}</span>
              </td>
              <td class="px-4 py-3 align-middle text-right">
                <a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={detailHref(row.id)}
                  data-testid="workflows-detail-link"
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
