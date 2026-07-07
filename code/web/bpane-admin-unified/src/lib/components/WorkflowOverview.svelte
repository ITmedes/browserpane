<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import {
    buildWorkflowOverviewModel,
    type WorkflowOverviewLoadState,
  } from '$lib/workflows/workflow-overview-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowCatalogTable from './WorkflowCatalogTable.svelte';

  type WorkflowOverviewProps = {
    readonly state: WorkflowOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: WorkflowOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildWorkflowOverviewModel(loadState.definitions, loadState.versions, loadState.hiddenCount)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="workflows-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflows</h1>
      {#if loadState.status === 'ready' && !loadState.includeHidden && loadState.hiddenCount > 0}
        <p class="m-0 mt-2 text-xs text-admin-muted" data-testid="workflows-hidden-note">
          {loadState.hiddenCount} internal workflow definition{loadState.hiddenCount === 1 ? '' : 's'} hidden from the operator catalog.
        </p>
      {/if}
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loadState.status === 'loading'}
      data-testid="workflows-refresh-button"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="workflows-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading workflows"
        message="The workflow catalog is being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Workflow catalog unavailable"
      message={loadState.message}
      testId="workflows-error"
    />
  {:else if loadState.definitions.length === 0}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="workflows-empty"
    >
      Workflow catalog is empty.
    </section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Workflow metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <WorkflowCatalogTable
      definitions={loadState.definitions}
      versions={loadState.versions}
      hiddenCount={loadState.hiddenCount}
    />
  {/if}
</div>
