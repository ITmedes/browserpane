<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildExtensionOverviewModel,
    extensionMatchesSearch,
    type ExtensionOverviewRow,
  } from '$lib/extensions/extension-view-model';
  import type { ExtensionDefinitionResource } from '$lib/extensions/extension-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type ExtensionLens = 'all' | 'enabled' | 'disabled' | 'versioned' | 'unversioned';

  type ExtensionCatalogTableProps = {
    readonly extensions: readonly ExtensionDefinitionResource[];
  };

  let { extensions }: ExtensionCatalogTableProps = $props();
  let lens = $state<ExtensionLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildExtensionOverviewModel(extensions));
  const visibleRows = $derived(filterRows(model.rows, lens, searchQuery));
  const lenses = $derived([
    { id: 'all' as const, label: 'All', count: model.rows.length },
    {
      id: 'enabled' as const,
      label: 'Enabled',
      count: model.rows.filter((row) => row.status === 'Enabled').length,
    },
    {
      id: 'disabled' as const,
      label: 'Disabled',
      count: model.rows.filter((row) => row.status === 'Disabled').length,
    },
    {
      id: 'versioned' as const,
      label: 'Versioned',
      count: model.rows.filter((row) => row.latestVersion !== 'No version').length,
    },
    {
      id: 'unversioned' as const,
      label: 'No version',
      count: model.rows.filter((row) => row.latestVersion === 'No version').length,
    },
  ]);

  function filterRows(
    rows: readonly ExtensionOverviewRow[],
    selectedLens: ExtensionLens,
    query: string,
  ): readonly ExtensionOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter(
      (row) => matchesLens(row, selectedLens) && extensionMatchesSearch(row, normalizedQuery),
    );
  }

  function matchesLens(row: ExtensionOverviewRow, selectedLens: ExtensionLens): boolean {
    if (selectedLens === 'enabled') return row.status === 'Enabled';
    if (selectedLens === 'disabled') return row.status === 'Disabled';
    if (selectedLens === 'versioned') return row.latestVersion !== 'No version';
    if (selectedLens === 'unversioned') return row.latestVersion === 'No version';
    return true;
  }
</script>

<section
  class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel"
  data-testid="extensions-list"
>
  <div
    class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between"
  >
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Approved extension catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">
        Deployment-managed unpacked browser extensions approved for future sessions and workflows.
      </p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="extensions-list-count"
      >{visibleRows.length} of {model.rows.length}</span
    >
  </div>

  <div
    class="flex flex-col border-b border-admin-border lg:flex-row lg:items-center lg:justify-between"
  >
    <div class="flex min-w-0 flex-wrap items-center px-4">
      {#each lenses as definition}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${lens === definition.id ? 'border-admin-accent text-admin-ink' : 'border-transparent text-admin-muted hover:text-admin-ink'}`}
          type="button"
          aria-pressed={lens === definition.id}
          onclick={() => {
            lens = definition.id;
          }}
          data-testid={`extensions-lens-${definition.id}`}
        >
          <span>{definition.label}</span>
          <span
            class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted"
            >{definition.count}</span
          >
        </button>
      {/each}
    </div>
    <label
      class="mx-4 mb-3 flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[340px]"
    >
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Search approved extensions</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Name, version, labels..."
        bind:value={searchQuery}
        data-testid="extensions-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto">
    <table class="w-full min-w-[860px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col"
            >Extension</th
          >
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col"
            >State</th
          >
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col"
            >Latest version</th
          >
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col"
            >Labels</th
          >
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col"
            >Updated</th
          >
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col"
            >Actions</th
          >
        </tr>
      </thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr
            ><td
              class="px-4 py-14 text-center text-sm text-admin-muted"
              colspan="6"
              data-testid="extensions-filter-empty">No extensions match the current filters.</td
            ></tr
          >
        {:else}
          {#each visibleRows as row}
            <tr
              class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft"
              data-testid="extensions-list-row"
            >
              <td class="w-[300px] px-4 py-3">
                <div class="grid min-w-0">
                  <span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span>
                  <span class="mt-1 truncate text-xs text-admin-muted">{row.description}</span>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                </div>
              </td>
              <td class="px-3 py-3"
                ><span
                  class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.statusTone)}`}
                  >{row.status}</span
                ></td
              >
              <td class="px-3 py-3"
                ><span
                  class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.versionTone)}`}
                  >{row.latestVersion}</span
                ></td
              >
              <td class="max-w-[220px] px-3 py-3 text-xs text-admin-muted"
                ><span class="line-clamp-2">{row.labels}</span></td
              >
              <td class="px-3 py-3 text-xs text-admin-muted">{row.updatedAt}</td>
              <td class="px-4 py-3 text-right">
                <a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={`/admin-new/extensions/${encodeURIComponent(row.id)}`}
                  data-testid="extensions-detail-link">Details</a
                >
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</section>
