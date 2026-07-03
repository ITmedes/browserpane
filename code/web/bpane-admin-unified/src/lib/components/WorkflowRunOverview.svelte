<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import {
    buildWorkflowRunOverviewModel,
    type WorkflowRunOverviewLoadState,
  } from '$lib/workflow-runs/workflow-run-overview-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowRunCatalogTable from './WorkflowRunCatalogTable.svelte';

  type WorkflowRunOverviewProps = {
    readonly state: WorkflowRunOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: WorkflowRunOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildWorkflowRunOverviewModel(loadState.runs)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="workflow-runs-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflow runs</h1>
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loadState.status === 'loading'}
      data-testid="workflow-runs-refresh"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="workflow-runs-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading workflow runs"
        message="The workflow-run catalog is being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Workflow run catalog unavailable"
      message={loadState.message}
      testId="workflow-runs-error"
    />
  {:else if loadState.runs.length === 0}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="workflow-runs-empty"
    >
      Workflow run catalog is empty.
    </section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Workflow run metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <WorkflowRunCatalogTable runs={loadState.runs} />
  {/if}
</div>
