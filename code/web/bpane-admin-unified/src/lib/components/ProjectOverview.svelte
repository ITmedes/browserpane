<script lang="ts">
  import { AlertTriangle, RefreshCw, Search } from '@lucide/svelte';
  import {
    buildProjectOverviewModel,
    type ProjectOverviewLoadState,
    type ProjectOverviewRow,
  } from '$lib/projects/project-overview-view-model';

  type ProjectLens = 'all' | 'active' | 'archived' | 'alerts';

  type ProjectLensDefinition = {
    readonly id: ProjectLens;
    readonly label: string;
    readonly count: number;
    readonly showDot?: boolean;
  };

  type ProjectOverviewProps = {
    readonly state: ProjectOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let { state: loadState, onRefresh }: ProjectOverviewProps = $props();
  let projectLens = $state<ProjectLens>('all');
  let searchQuery = $state('');

  const model = $derived(loadState.status === 'ready'
    ? buildProjectOverviewModel(loadState.projects)
    : null);
  const lensDefinitions = $derived(model ? buildProjectLensDefinitions(model.rows) : []);
  const visibleRows = $derived(model ? filterProjectRows(model.rows, projectLens, searchQuery) : []);

  function refresh(): void {
    void onRefresh?.();
  }

  function buildProjectLensDefinitions(rows: readonly ProjectOverviewRow[]): readonly ProjectLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: rows.length },
      {
        id: 'active',
        label: 'Active',
        count: rows.filter((row) => row.state === 'active').length,
        showDot: true,
      },
      { id: 'archived', label: 'Archived', count: rows.filter((row) => row.state === 'archived').length },
      { id: 'alerts', label: 'Needs attention', count: rows.filter((row) => row.alertTone !== 'neutral').length },
    ];
  }

  function filterProjectRows(
    rows: readonly ProjectOverviewRow[],
    lens: ProjectLens,
    query: string,
  ): readonly ProjectOverviewRow[] {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, lens) && matchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: ProjectOverviewRow, lens: ProjectLens): boolean {
    if (lens === 'active') {
      return row.state === 'active';
    }
    if (lens === 'archived') {
      return row.state === 'archived';
    }
    if (lens === 'alerts') {
      return row.alertTone !== 'neutral';
    }
    return true;
  }

  function matchesSearch(row: ProjectOverviewRow, normalizedQuery: string): boolean {
    if (!normalizedQuery) {
      return true;
    }
    return [
      row.id,
      row.name,
      row.description,
      row.state,
      row.activeSessions,
      row.activeWorkflowRuns,
      row.policySummary,
      row.alerts,
    ].some((value) => value.toLowerCase().includes(normalizedQuery));
  }

  function toneClass(tone: 'success' | 'neutral' | 'warning' | 'danger'): string {
    if (tone === 'success') {
      return 'bg-emerald-50 text-emerald-700 ring-emerald-200';
    }
    if (tone === 'warning') {
      return 'bg-amber-50 text-amber-700 ring-amber-200';
    }
    if (tone === 'danger') {
      return 'bg-red-50 text-red-700 ring-red-200';
    }
    return 'bg-slate-100 text-slate-600 ring-slate-200';
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="projects-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Projects</h1>
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loadState.status === 'loading'}
      data-testid="projects-refresh-button"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="projects-loading"
    >
      Loading projects...
    </section>
  {:else if loadState.status === 'error'}
    <section
      class="flex items-start gap-3 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-900"
      role="alert"
      data-testid="projects-error"
    >
      <AlertTriangle class="mt-0.5 shrink-0" size={17} strokeWidth={1.9} />
      <div class="min-w-0">
        <p class="m-0 font-semibold">Project catalog unavailable</p>
        <p class="m-0 mt-1 break-words text-red-800">{loadState.message}</p>
      </div>
    </section>
  {:else if loadState.projects.length === 0}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="projects-empty"
    >
      Project catalog is empty.
    </section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Project metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <section class="min-h-0 rounded-md border border-admin-border bg-admin-panel" data-testid="projects-list">
      <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
        <div class="min-w-0">
          <h2 class="m-0 text-sm font-semibold text-admin-ink">Project catalog</h2>
          <p class="m-0 mt-1 text-xs text-admin-muted">Governance scope, policy gates, usage, and quota signals.</p>
        </div>
        <span class="text-xs text-admin-muted" data-testid="projects-list-count">
          {visibleRows.length} of {model.rows.length}
        </span>
      </div>

      <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
        <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
          {#each lensDefinitions as lens}
            <button
              class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
                projectLens === lens.id
                  ? 'border-admin-accent text-admin-ink'
                  : 'border-transparent text-admin-muted hover:text-admin-ink'
              }`}
              type="button"
              aria-pressed={projectLens === lens.id}
              onclick={() => {
                projectLens = lens.id;
              }}
              data-testid={`projects-lens-${lens.id}`}
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
          <span class="sr-only">Search projects</span>
          <input
            class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
            type="search"
            placeholder="Project, state, policy, usage..."
            bind:value={searchQuery}
            data-testid="projects-search"
          />
        </label>
      </div>

      <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
        <table class="w-full min-w-[1120px] border-collapse">
          <thead class="sticky top-0 z-10 bg-admin-soft">
            <tr class="border-b border-admin-border">
              <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Project</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Activity</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Runtime</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Egress</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Storage</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Policy</th>
              <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Alerts</th>
              <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Updated</th>
            </tr>
          </thead>
          <tbody>
            {#if visibleRows.length === 0}
              <tr>
                <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="9" data-testid="projects-filter-empty">
                  No projects match the current filters.
                </td>
              </tr>
            {:else}
              {#each visibleRows as row}
                <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="projects-list-row">
                  <td class="w-[260px] px-4 py-3 align-middle">
                    <div class="min-w-0">
                      <p class="m-0 truncate text-sm font-semibold text-admin-ink">{row.name}</p>
                      <p class="m-0 mt-1 truncate text-xs text-admin-muted">{row.description}</p>
                      <p class="m-0 mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</p>
                    </div>
                  </td>
                  <td class="px-3 py-3 align-middle">
                    <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${toneClass(row.stateTone)}`}>
                      {row.state}
                    </span>
                  </td>
                  <td class="px-3 py-3 align-middle text-xs text-admin-muted">
                    <div class="grid gap-1">
                      <span><span class="font-semibold text-admin-ink">{row.activeSessions}</span> sessions</span>
                      <span>{row.queuedSessions} queued</span>
                      <span>{row.activeWorkflowRuns} runs</span>
                      <span>{row.sessionCreations} created</span>
                    </div>
                  </td>
                  <td class="px-3 py-3 align-middle font-mono text-xs text-admin-ink">{row.runtimeUsage}</td>
                  <td class="px-3 py-3 align-middle font-mono text-xs text-admin-ink">{row.egressUsage}</td>
                  <td class="px-3 py-3 align-middle font-mono text-xs text-admin-ink">{row.retainedStorage}</td>
                  <td class="max-w-[220px] px-3 py-3 align-middle text-xs text-admin-muted">
                    <span class="line-clamp-2">{row.policySummary}</span>
                  </td>
                  <td class="px-3 py-3 align-middle">
                    <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${toneClass(row.alertTone)}`}>
                      {row.alerts}
                    </span>
                  </td>
                  <td class="px-4 py-3 align-middle text-xs text-admin-muted">{row.updatedAt}</td>
                </tr>
              {/each}
            {/if}
          </tbody>
        </table>
      </div>
    </section>
  {/if}
</div>
