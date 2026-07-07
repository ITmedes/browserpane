<script lang="ts">
  import { Plus, RefreshCw } from '@lucide/svelte';
  import {
    buildProjectOverviewModel,
    type ProjectOverviewLoadState,
  } from '$lib/projects/project-overview-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import ProjectCatalogTable from './ProjectCatalogTable.svelte';

  type ProjectOverviewProps = {
    readonly state: ProjectOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: ProjectOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildProjectOverviewModel(loadState.projects)
    : null);
  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="projects-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Projects</h1>
    </div>

    <div class="flex flex-wrap gap-2">
      <a
        class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2"
        href="/admin-new/projects/new"
        data-testid="projects-new-link"
      >
        <Plus size={16} strokeWidth={1.9} />
        <span>New project</span>
      </a>
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
    </div>
  </header>

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="projects-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading projects"
        message="The project catalog is being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Project catalog unavailable"
      message={loadState.message}
      testId="projects-error"
    />
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

    <ProjectCatalogTable projects={loadState.projects} />
  {/if}
</div>
