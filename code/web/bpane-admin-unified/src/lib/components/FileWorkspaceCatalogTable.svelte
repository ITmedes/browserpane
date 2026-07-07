<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildFileWorkspaceOverviewModel,
    fileWorkspaceMatchesSearch,
    type FileWorkspaceFileCountMap,
    type FileWorkspaceOverviewRow,
  } from '$lib/file-workspaces/file-workspace-overview-view-model';
  import type { FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type FileWorkspaceLens = 'all' | 'owner' | 'project' | 'with-files' | 'empty' | 'attention';

  type FileWorkspaceLensDefinition = {
    readonly id: FileWorkspaceLens;
    readonly label: string;
    readonly count: number;
  };

  type FileWorkspaceCatalogTableProps = {
    readonly workspaces: readonly FileWorkspaceResource[];
    readonly fileCounts?: FileWorkspaceFileCountMap;
  };

  let {
    workspaces,
    fileCounts = {},
  }: FileWorkspaceCatalogTableProps = $props();
  let workspaceLens = $state<FileWorkspaceLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildFileWorkspaceOverviewModel(workspaces, fileCounts));
  const lensDefinitions = $derived(buildLensDefinitions(model.rows));
  const visibleRows = $derived(filterRows(model.rows, workspaceLens, searchQuery));

  function buildLensDefinitions(rows: readonly FileWorkspaceOverviewRow[]): readonly FileWorkspaceLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      { id: 'owner', label: 'Owner scoped', count: rows.filter((row) => row.scopeTone === 'neutral').length },
      { id: 'project', label: 'Project scoped', count: rows.filter((row) => row.scopeTone === 'warning').length },
      { id: 'with-files', label: 'With files', count: rows.filter((row) => row.fileCount !== null && row.fileCount > 0).length },
      { id: 'empty', label: 'Empty', count: rows.filter((row) => row.fileCount === 0).length },
      { id: 'attention', label: 'Needs attention', count: rows.filter((row) => row.fileCount === null).length },
    ];
  }

  function filterRows(
    rows: readonly FileWorkspaceOverviewRow[],
    lens: FileWorkspaceLens,
    query: string,
  ): readonly FileWorkspaceOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, lens) && fileWorkspaceMatchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: FileWorkspaceOverviewRow, lens: FileWorkspaceLens): boolean {
    if (lens === 'owner') {
      return row.scopeTone === 'neutral';
    }
    if (lens === 'project') {
      return row.scopeTone === 'warning';
    }
    if (lens === 'with-files') {
      return row.fileCount !== null && row.fileCount > 0;
    }
    if (lens === 'empty') {
      return row.fileCount === 0;
    }
    if (lens === 'attention') {
      return row.fileCount === null;
    }
    return true;
  }

  function detailHref(workspaceId: string): string {
    return `/admin-new/files/workspaces/${encodeURIComponent(workspaceId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="file-workspaces-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">File workspace catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Reusable input and output file collections for sessions and workflows.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="file-workspaces-list-count">
      {visibleRows.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            workspaceLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={workspaceLens === lens.id}
          onclick={() => {
            workspaceLens = lens.id;
          }}
          data-testid={`file-workspaces-lens-${lens.id}`}
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
      <span class="sr-only">Search file workspaces</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Workspace, scope, labels..."
        bind:value={searchQuery}
        data-testid="file-workspaces-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[920px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Workspace</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Scope</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Files</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Labels</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Updated</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="6" data-testid="file-workspaces-filter-empty">
              No file workspaces match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleRows as row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="file-workspaces-list-row">
              <td class="w-[310px] px-4 py-3 align-middle">
                <div class="grid min-w-0 text-left">
                  <span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span>
                  <span class="mt-1 truncate text-xs text-admin-muted">{row.description}</span>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                </div>
              </td>
              <td class="max-w-[180px] px-3 py-3 align-middle">
                <span class={`inline-flex max-w-full rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.scopeTone)}`}>
                  <span class="truncate">{row.scope}</span>
                </span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.fileCountTone)}`}>
                  {row.fileCountLabel}
                </span>
              </td>
              <td class="max-w-[240px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.labels}</span>
              </td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.updatedAt}</td>
              <td class="px-4 py-3 align-middle text-right">
                <a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={detailHref(row.id)}
                  data-testid="file-workspaces-detail-link"
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
