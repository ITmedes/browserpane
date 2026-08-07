<script lang="ts">
  import { RefreshCw, Upload } from '@lucide/svelte';
  import {
    buildBrowserContextOverviewModel,
    type BrowserContextOverviewLoadState,
  } from '$lib/browser-contexts/browser-context-overview-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import BrowserContextCatalogTable from './BrowserContextCatalogTable.svelte';

  type BrowserContextOverviewProps = {
    readonly state: BrowserContextOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: BrowserContextOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildBrowserContextOverviewModel(loadState.contexts)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="browser-contexts-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Browser contexts</h1>
    </div>

    <div class="flex flex-wrap gap-2">
      <a
        class="inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2"
        href="/admin-new/browser-contexts/new"
        data-testid="browser-contexts-new-link"
      >
        New browser context
      </a>
      <a
        class="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm transition hover:bg-admin-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25"
        href="/admin-new/browser-contexts/import"
        data-testid="browser-contexts-import-link"
      >
        <Upload size={16} strokeWidth={1.9} />
        <span>Import archive</span>
      </a>
      <button
        class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        onclick={refresh}
        disabled={loadState.status === 'loading'}
        data-testid="browser-contexts-refresh-button"
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
      data-testid="browser-contexts-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading browser contexts"
        message="The browser context catalog is being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Browser context catalog unavailable"
      message={loadState.message}
      testId="browser-contexts-error"
    />
  {:else if loadState.contexts.length === 0}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="browser-contexts-empty"
    >
      Browser context catalog is empty.
    </section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Browser context metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <BrowserContextCatalogTable contexts={loadState.contexts} />
  {/if}
</div>
