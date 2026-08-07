<script lang="ts">
  import { Plus, RefreshCw } from '@lucide/svelte';
  import {
    buildWorkflowEventOverviewModel,
    type WorkflowEventOverviewLoadState,
  } from '$lib/workflow-events/workflow-event-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowEventSubscriptionCatalogTable from './WorkflowEventSubscriptionCatalogTable.svelte';
  let {
    state: loadState,
    onRefresh,
  }: {
    readonly state: WorkflowEventOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  } = $props();
  const model = $derived(
    loadState.status === 'ready' ? buildWorkflowEventOverviewModel(loadState.subscriptions) : null,
  );
</script>

<div
  class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
  data-testid="workflow-event-subscriptions-overview"
>
  <header
    class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between"
  >
    <div>
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Govern</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflow event subscriptions</h1>
    </div>
    <div class="flex flex-wrap gap-2">
      <a
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90"
        href="/admin-new/workflow-event-subscriptions/new"
        data-testid="workflow-event-subscriptions-new-link"
        ><Plus size={16} strokeWidth={1.9} /><span>New subscription</span></a
      ><button
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:opacity-60"
        type="button"
        onclick={() => void onRefresh?.()}
        disabled={loadState.status === 'loading'}
        data-testid="workflow-event-subscriptions-refresh-button"
        ><RefreshCw size={16} strokeWidth={1.9} /><span>Refresh</span></button
      >
    </div>
  </header>
  {#if loadState.status === 'loading'}<section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel"
      data-testid="workflow-event-subscriptions-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading workflow event subscriptions"
        message="Subscription metadata is being refreshed from the control API."
      />
    </section>{:else if loadState.status === 'error'}<AdminMessage
      tone="error"
      title="Event subscription catalog unavailable"
      message={loadState.message}
      testId="workflow-event-subscriptions-error"
    />{:else if loadState.subscriptions.length === 0}<section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="workflow-event-subscriptions-empty"
    >
      Workflow event subscription catalog is empty.
    </section>{:else if model}<section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {#each model.metrics as metric}<div
          class="rounded-md border border-admin-border bg-admin-panel p-4"
          data-testid={metric.testId}
        >
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>{/each}
    </section>
    <WorkflowEventSubscriptionCatalogTable subscriptions={loadState.subscriptions} />{/if}
</div>
