<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildSessionOverviewModel,
    type SessionOverviewRow,
  } from '$lib/sessions/session-overview-view-model';
  import type { SessionResource } from '$lib/sessions/session-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type SessionLens = 'all' | 'active' | 'queued' | 'stopped' | 'attention';

  type SessionLensDefinition = {
    readonly id: SessionLens;
    readonly label: string;
    readonly count: number;
    readonly showDot?: boolean;
  };

  type SessionCatalogTableProps = {
    readonly sessions: readonly SessionResource[];
  };

  let { sessions }: SessionCatalogTableProps = $props();
  let sessionLens = $state<SessionLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildSessionOverviewModel(sessions));
  const lensDefinitions = $derived(buildSessionLensDefinitions(model.rows));
  const visibleRows = $derived(filterSessionRows(model.rows, sessionLens, searchQuery));

  function buildSessionLensDefinitions(rows: readonly SessionOverviewRow[]): readonly SessionLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      {
        id: 'active',
        label: 'Active',
        count: rows.filter((row) => !['stopped', 'released', 'cancelled', 'failed'].includes(row.state)).length,
        showDot: true,
      },
      { id: 'queued', label: 'Queued', count: rows.filter((row) => row.state === 'queued').length },
      {
        id: 'stopped',
        label: 'Stopped',
        count: rows.filter((row) => ['stopped', 'released', 'cancelled'].includes(row.state)).length,
      },
      { id: 'attention', label: 'Needs attention', count: rows.filter((row) => row.attention !== 'none').length },
    ];
  }

  function filterSessionRows(
    rows: readonly SessionOverviewRow[],
    lens: SessionLens,
    query: string,
  ): readonly SessionOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, lens) && matchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: SessionOverviewRow, lens: SessionLens): boolean {
    if (lens === 'active') {
      return !['stopped', 'released', 'cancelled', 'failed'].includes(row.state);
    }
    if (lens === 'queued') {
      return row.state === 'queued';
    }
    if (lens === 'stopped') {
      return ['stopped', 'released', 'cancelled'].includes(row.state);
    }
    if (lens === 'attention') {
      return row.attention !== 'none';
    }
    return true;
  }

  function matchesSearch(row: SessionOverviewRow, normalizedQuery: string): boolean {
    if (!normalizedQuery) {
      return true;
    }
    return [
      row.id,
      row.shortId,
      row.state,
      row.runtime,
      row.presence,
      row.project,
      row.admission,
      row.template,
      row.browserContext,
      row.egress,
      row.capabilities,
      row.attention,
    ].some((value) => value.toLowerCase().includes(normalizedQuery));
  }

  function detailHref(sessionId: string): string {
    return `/admin-new/sessions/${encodeURIComponent(sessionId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="sessions-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Session catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Lifecycle, runtime, project, browser-context, egress, and connection state.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="sessions-list-count">
      {visibleRows.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            sessionLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={sessionLens === lens.id}
          onclick={() => {
            sessionLens = lens.id;
          }}
          data-testid={`sessions-lens-${lens.id}`}
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
      <span class="sr-only">Search sessions</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Session, state, project, egress..."
        bind:value={searchQuery}
        data-testid="sessions-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[1180px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Session</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Runtime</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Connections</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Project</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Resources</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Capabilities</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Attention</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Updated</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="10" data-testid="sessions-filter-empty">
              No sessions match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleRows as row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="sessions-list-row">
              <td class="w-[250px] px-4 py-3 align-middle">
                <div class="grid min-w-0 text-left">
                  <span class="truncate font-mono text-sm font-semibold text-admin-ink" title={row.id}>{row.shortId}</span>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                </div>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>
                  {row.state}
                </span>
              </td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">
                <div class="grid gap-1">
                  <span class="font-semibold text-admin-ink">{row.runtime}</span>
                  <span>{row.presence}</span>
                </div>
              </td>
              <td class="px-3 py-3 align-middle font-mono text-xs text-admin-ink">{row.clients}</td>
              <td class="max-w-[190px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.project}</span>
                <span class="mt-1 block truncate">{row.admission}</span>
              </td>
              <td class="max-w-[230px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="block truncate">{row.template}</span>
                <span class="block truncate">{row.browserContext}</span>
                <span class="block truncate">{row.egress}</span>
              </td>
              <td class="max-w-[220px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.capabilities}</span>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.attentionTone)}`}>
                  {row.attention}
                </span>
              </td>
              <td class="px-3 py-3 align-middle text-xs text-admin-muted">{row.updatedAt}</td>
              <td class="px-4 py-3 align-middle text-right">
                <a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={detailHref(row.id)}
                  data-testid="sessions-detail-link"
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
