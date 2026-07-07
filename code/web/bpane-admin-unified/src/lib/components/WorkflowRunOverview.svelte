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

  const restExample = `POST /api/v1/workflow-runs
Authorization: Bearer <admin-access-token>
Content-Type: application/json

{
  "workflow_id": "<workflow-id>",
  "version": "v1",
  "session": { "create_session": {} },
  "input": { "target_url": "https://example.com" },
  "client_request_id": "external-system-unique-id"
}`;
  const cliExample = `npm run workflow:cli -- workflow run create \\
  --workflow-id <workflow-id> \\
  --version v1 \\
  --create-session \\
  --input-json '{"target_url":"https://example.com"}'`;
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

  <section
    class="grid gap-3 rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]"
    data-testid="workflow-runs-integration-panel"
  >
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Integration</p>
      <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Start workflow runs from outside the admin app</h2>
      <p class="m-0 mt-2 text-sm leading-6 text-admin-muted">
        External schedulers, CI jobs, or service integrations should create a workflow run with a stable
        <code class="font-mono text-admin-ink">client_request_id</code>, then poll
        <code class="font-mono text-admin-ink">/api/v1/workflow-runs/&lbrace;run_id&rbrace;</code>
        or subscribe to run events and logs.
      </p>
    </div>

    <div class="grid min-w-0 gap-3 md:grid-cols-2 lg:grid-cols-1 xl:grid-cols-2">
      <div class="min-w-0 rounded-md border border-admin-border bg-admin-soft/60 p-3">
        <p class="m-0 text-xs font-semibold uppercase text-admin-muted">REST</p>
        <pre class="mt-2 max-h-48 overflow-auto rounded-md border border-admin-border bg-white p-3 text-xs leading-5 text-admin-ink" data-testid="workflow-runs-rest-example">{restExample}</pre>
      </div>
      <div class="min-w-0 rounded-md border border-admin-border bg-admin-soft/60 p-3">
        <p class="m-0 text-xs font-semibold uppercase text-admin-muted">CLI</p>
        <pre class="mt-2 max-h-48 overflow-auto rounded-md border border-admin-border bg-white p-3 text-xs leading-5 text-admin-ink" data-testid="workflow-runs-cli-example">{cliExample}</pre>
      </div>
    </div>
  </section>

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
