<script lang="ts">
  import { AlertTriangle, RefreshCw } from '@lucide/svelte';
  import {
    buildProjectOverviewModel,
    type ProjectOverviewLoadState,
  } from '$lib/projects/project-overview-view-model';

  type ProjectOverviewProps = {
    readonly state: ProjectOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let { state, onRefresh }: ProjectOverviewProps = $props();

  const model = $derived(state.status === 'ready'
    ? buildProjectOverviewModel(state.projects, state.selectedProjectId)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="projects-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase tracking-[0.14em] text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Projects</h1>
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={state.status === 'loading'}
      data-testid="projects-refresh-button"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  {#if state.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="projects-loading"
    >
      Loading projects...
    </section>
  {:else if state.status === 'error'}
    <section
      class="flex items-start gap-3 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-900"
      role="alert"
      data-testid="projects-error"
    >
      <AlertTriangle class="mt-0.5 shrink-0" size={17} strokeWidth={1.9} />
      <div class="min-w-0">
        <p class="m-0 font-semibold">Project catalog unavailable</p>
        <p class="m-0 mt-1 break-words text-red-800">{state.message}</p>
      </div>
    </section>
  {:else if state.projects.length === 0}
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
          <p class="m-0 text-xs font-semibold uppercase tracking-[0.1em] text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <section class="grid min-h-0 gap-5 xl:grid-cols-[minmax(320px,0.9fr)_minmax(0,1.4fr)]">
      <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="projects-list">
        <div class="border-b border-admin-border px-4 py-3">
          <h2 class="m-0 text-sm font-semibold text-admin-ink">Project catalog</h2>
        </div>
        <div class="max-h-[calc(100vh-260px)] min-h-64 overflow-y-auto">
          {#each model.rows as row}
            <a
              class={`grid min-w-0 gap-2 border-b border-admin-border px-4 py-3 text-left last:border-b-0 ${
                row.selected ? 'bg-[#eef2ff]' : 'hover:bg-admin-soft'
              }`}
              href={`/admin-new/projects?project=${encodeURIComponent(row.id)}`}
              data-testid="projects-list-row"
            >
              <div class="flex min-w-0 items-center justify-between gap-3">
                <span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span>
                <span
                  class={`shrink-0 rounded-full px-2 py-0.5 text-xs font-semibold ${
                    row.state === 'active' ? 'bg-emerald-50 text-emerald-700' : 'bg-slate-100 text-slate-600'
                  }`}
                >
                  {row.state}
                </span>
              </div>
              <p class="m-0 truncate text-xs text-admin-muted">{row.description}</p>
              <div class="grid gap-2 text-xs text-admin-muted sm:grid-cols-2">
                <span>Sessions: {row.activeSessions}</span>
                <span>Runs: {row.activeWorkflowRuns}</span>
                <span>Alerts: {row.alerts}</span>
                <span class="truncate">{row.policySummary}</span>
              </div>
            </a>
          {/each}
        </div>
      </div>

      <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="projects-selected-detail">
        {#if model.selected}
          <div class="border-b border-admin-border px-4 py-4">
            <p class="m-0 text-xs font-semibold uppercase tracking-[0.1em] text-admin-muted">Selected project</p>
            <h2 class="m-0 mt-1 truncate text-xl font-semibold text-admin-ink">{model.selected.name}</h2>
            {#if model.selected.description}
              <p class="m-0 mt-1 text-sm text-admin-muted">{model.selected.description}</p>
            {/if}
          </div>

          <div class="grid gap-5 p-4 lg:grid-cols-3">
            {#each model.selectedSections as section}
              <section class="min-w-0" aria-label={section.title}>
                <h3 class="m-0 text-sm font-semibold text-admin-ink">{section.title}</h3>
                <dl class="mt-3 grid gap-3">
                  {#each section.rows as row}
                    <div class="min-w-0">
                      <dt class="text-xs font-semibold uppercase tracking-[0.08em] text-admin-muted">{row.label}</dt>
                      <dd class="m-0 mt-1 break-words text-sm text-admin-ink">{row.value}</dd>
                    </div>
                  {/each}
                </dl>
              </section>
            {/each}
          </div>
        {/if}
      </div>
    </section>
  {/if}
</div>
