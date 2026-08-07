<script lang="ts">
  import { Plus, RefreshCw } from '@lucide/svelte';
  import { buildCredentialBindingOverviewModel, type CredentialBindingOverviewLoadState } from '$lib/credential-bindings/credential-binding-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import CredentialBindingCatalogTable from './CredentialBindingCatalogTable.svelte';

  let { state: loadState, onRefresh }: { readonly state: CredentialBindingOverviewLoadState; readonly onRefresh?: () => void | Promise<void> } = $props();
  const model = $derived(loadState.status === 'ready' ? buildCredentialBindingOverviewModel(loadState.bindings) : null);
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="credential-bindings-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div><p class="m-0 text-xs font-semibold uppercase text-admin-muted">Resources</p><h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Credential bindings</h1></div>
    <div class="flex flex-wrap gap-2"><a class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm hover:opacity-90" href="/admin-new/credential-bindings/new" data-testid="credential-bindings-new-link"><Plus size={16} strokeWidth={1.9} /><span>New binding</span></a><button class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:opacity-60" type="button" onclick={() => void onRefresh?.()} disabled={loadState.status === 'loading'} data-testid="credential-bindings-refresh-button"><RefreshCw size={16} strokeWidth={1.9} /><span>Refresh</span></button></div>
  </header>
  {#if loadState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel" data-testid="credential-bindings-loading"><AdminMessage tone="loading" title="Loading credential bindings" message="Safe binding metadata is being refreshed from the control API." /></section>
  {:else if loadState.status === 'error'}
    <AdminMessage tone="error" title="Credential binding catalog unavailable" message={loadState.message} testId="credential-bindings-error" />
  {:else if loadState.bindings.length === 0}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted" data-testid="credential-bindings-empty">Credential binding catalog is empty.</section>
  {:else if model}
    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Credential binding metrics">{#each model.metrics as metric}<div class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid={metric.testId}><p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p><p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p></div>{/each}</section>
    <CredentialBindingCatalogTable bindings={loadState.bindings} />
  {/if}
</div>
