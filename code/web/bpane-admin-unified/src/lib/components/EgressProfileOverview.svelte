<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import {
    buildEgressProfileOverviewModel,
    type EgressProfileOverviewLoadState,
  } from '$lib/egress-profiles/egress-profile-overview-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import EgressProfileCatalogTable from './EgressProfileCatalogTable.svelte';

  type EgressProfileOverviewProps = {
    readonly state: EgressProfileOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: EgressProfileOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildEgressProfileOverviewModel(loadState.profiles)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="egress-profiles-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Egress profiles</h1>
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loadState.status === 'loading'}
      data-testid="egress-profiles-refresh-button"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="egress-profiles-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading egress profiles"
        message="The egress catalog is being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Egress profile catalog unavailable"
      message={loadState.message}
      testId="egress-profiles-error"
    />
  {:else if loadState.profiles.length === 0}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      data-testid="egress-profiles-empty"
    >
      Egress profile catalog is empty.
    </section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Egress profile metrics">
      {#each model.metrics as metric}
        <div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
          <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
        </div>
      {/each}
    </section>

    <EgressProfileCatalogTable profiles={loadState.profiles} />
  {/if}
</div>
