<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildEgressProfileOverviewModel,
    egressProfileMatchesSearch,
    type EgressProfileKind,
    type EgressProfileOverviewRow,
  } from '$lib/egress-profiles/egress-profile-overview-view-model';
  import type { EgressProfileResource } from '$lib/egress-profiles/egress-profile-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type EgressProfileLens = 'all' | 'ready' | 'disabled' | 'attention' | EgressProfileKind;

  type EgressProfileLensDefinition = {
    readonly id: EgressProfileLens;
    readonly label: string;
    readonly count: number;
    readonly showDot?: boolean;
  };

  type EgressProfileCatalogTableProps = {
    readonly profiles: readonly EgressProfileResource[];
  };

  let {
    profiles,
  }: EgressProfileCatalogTableProps = $props();
  let profileLens = $state<EgressProfileLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildEgressProfileOverviewModel(profiles));
  const lensDefinitions = $derived(buildLensDefinitions(model.rows));
  const visibleRows = $derived(filterRows(model.rows, profileLens, searchQuery));

  function buildLensDefinitions(rows: readonly EgressProfileOverviewRow[]): readonly EgressProfileLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      {
        id: 'ready',
        label: 'Ready',
        count: rows.filter((row) => row.state === 'ready').length,
        showDot: true,
      },
      { id: 'disabled', label: 'Disabled', count: rows.filter((row) => row.state === 'disabled').length },
      { id: 'attention', label: 'Needs attention', count: rows.filter((row) => row.healthTone !== 'success').length },
      { id: 'proxy', label: 'Proxy', count: rows.filter((row) => row.kind === 'proxy').length },
      { id: 'tls', label: 'TLS', count: rows.filter((row) => row.kind === 'tls').length },
      { id: 'direct', label: 'Direct', count: rows.filter((row) => row.kind === 'direct').length },
    ];
  }

  function filterRows(
    rows: readonly EgressProfileOverviewRow[],
    lens: EgressProfileLens,
    query: string,
  ): readonly EgressProfileOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, lens) && egressProfileMatchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: EgressProfileOverviewRow, lens: EgressProfileLens): boolean {
    if (lens === 'ready') {
      return row.state === 'ready';
    }
    if (lens === 'disabled') {
      return row.state === 'disabled';
    }
    if (lens === 'attention') {
      return row.healthTone !== 'success';
    }
    if (lens === 'proxy' || lens === 'tls' || lens === 'direct') {
      return row.kind === lens;
    }
    return true;
  }

  function detailHref(profileId: string): string {
    return `/admin-new/egress/${encodeURIComponent(profileId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="egress-profiles-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Egress profile catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Outbound proxy, TLS inspection, diagnostics, and project-scope posture.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="egress-profiles-list-count">
      {visibleRows.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            profileLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={profileLens === lens.id}
          onclick={() => {
            profileLens = lens.id;
          }}
          data-testid={`egress-profiles-lens-${lens.id}`}
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
      <span class="sr-only">Search egress profiles</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Profile, health, scope, proxy, TLS..."
        bind:value={searchQuery}
        data-testid="egress-profiles-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[1120px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Profile</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Kind</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Scope</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Proxy</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Observation</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Health</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Proof</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Updated</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="10" data-testid="egress-profiles-filter-empty">
              No egress profiles match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleRows as row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="egress-profiles-list-row">
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
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.kindTone)}`}>
                  {row.kindLabel}
                </span>
              </td>
              <td class="max-w-[170px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.scope}</span>
              </td>
              <td class="max-w-[250px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.proxySummary}</span>
              </td>
              <td class="max-w-[210px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.observationSummary}</span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.healthTone)}`}>
                  {row.healthLabel}
                </span>
              </td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.proofLabel}</td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.updatedAt}</td>
              <td class="px-4 py-3 align-middle text-right">
                <a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={detailHref(row.id)}
                  data-testid="egress-profiles-detail-link"
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
