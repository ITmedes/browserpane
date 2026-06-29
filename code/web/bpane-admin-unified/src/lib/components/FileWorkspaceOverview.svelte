<script lang="ts">
  import { Plus, RefreshCw } from '@lucide/svelte';
  import {
    buildFileWorkspaceOverviewModel,
    type FileWorkspaceOverviewLoadState,
  } from '$lib/file-workspaces/file-workspace-overview-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import FileWorkspaceCatalogTable from './FileWorkspaceCatalogTable.svelte';

  type FileWorkspaceOverviewProps = {
    readonly state: FileWorkspaceOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: FileWorkspaceOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildFileWorkspaceOverviewModel(loadState.workspaces, loadState.fileCounts)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="file-workspaces-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">File workspaces</h1>
    </div>

    <div class="flex flex-wrap gap-2">
      <a
        class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2"
        href="/admin-new/files/workspaces/new"
        data-testid="file-workspaces-new-link"
      >
        <Plus size={16} strokeWidth={1.9} />
        <span>New workspace</span>
      </a>
      <button
        class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={refresh}
        disabled={loadState.status === 'loading'}
        data-testid="file-workspaces-refresh-button"
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
      data-testid="file-workspaces-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading file workspaces"
        message="The file workspace catalog is being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="File workspace catalog unavailable"
      message={loadState.message}
      testId="file-workspaces-error"
    />
  {:else if loadState.workspaces.length === 0}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="file-workspaces-empty"
    >
      File workspace catalog is empty.
    </section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="File workspace metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <FileWorkspaceCatalogTable workspaces={loadState.workspaces} fileCounts={loadState.fileCounts} />
  {/if}
</div>
