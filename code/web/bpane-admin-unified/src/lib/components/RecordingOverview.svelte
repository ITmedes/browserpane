<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import {
    buildRecordingOverviewModel,
    type RecordingActionState,
    type RecordingOverviewLoadState,
  } from '$lib/recordings/recording-overview-view-model';
  import type { RecordingCatalogEntry } from '$lib/recordings/recording-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import RecordingCatalogTable from './RecordingCatalogTable.svelte';

  type RecordingOverviewProps = {
    readonly state: RecordingOverviewLoadState;
    readonly actionState?: RecordingActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onDownloadRecording?: (entry: RecordingCatalogEntry) => void | Promise<void>;
  };

  let {
    state: loadState,
    actionState = { status: 'idle' },
    onRefresh,
    onDownloadRecording,
  }: RecordingOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildRecordingOverviewModel(loadState.entries)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="recordings-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Recordings</h1>
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loadState.status === 'loading' || actionState.status === 'running'}
      data-testid="recordings-refresh"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  <ActionFeedback
    state={actionState}
    successTitle="Recording action completed"
    errorTitle="Recording action failed"
    successTestId="recordings-action-success"
    errorTestId="recordings-action-error"
    runningTestId="recordings-action-running"
  />

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="recordings-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading recordings"
        message="The recording catalog is being assembled from visible sessions."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Recording catalog unavailable"
      message={loadState.message}
      testId="recordings-error"
    />
  {:else if loadState.entries.length === 0}
    {#if loadState.failures.length > 0}
      <AdminMessage
        tone="warning"
        title="Partial recording catalog"
        message={`${loadState.failures.length} session recording list could not be loaded.`}
        items={loadState.failures.slice(0, 4).map((failure) => `${failure.sessionId}: ${failure.message}`)}
        testId="recordings-partial-warning"
      />
    {/if}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="recordings-empty"
    >
      No session recordings are available.
    </section>
  {:else if model}
    {#if loadState.failures.length > 0}
      <AdminMessage
        tone="warning"
        title="Partial recording catalog"
        message={`${loadState.failures.length} session recording list could not be loaded.`}
        items={loadState.failures.slice(0, 4).map((failure) => `${failure.sessionId}: ${failure.message}`)}
        testId="recordings-partial-warning"
      />
    {/if}

    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Recording metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <RecordingCatalogTable
      entries={loadState.entries}
      busy={actionState.status === 'running'}
      {onDownloadRecording}
    />
  {/if}
</div>
