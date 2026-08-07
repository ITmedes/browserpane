<script lang="ts">
  import { Bot, RefreshCw, ShieldCheck, Waypoints } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import { buildIdentityReviewViewModel, type IdentityReviewLoadState } from '$lib/identity/identity-review-view-model';
  import type { UpsertIdentityMappingRequest, UpsertServicePrincipalRequest } from '$lib/identity/identity-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import IdentityMappingCatalog from './IdentityMappingCatalog.svelte';
  import IdentityReviewSummary from './IdentityReviewSummary.svelte';
  import ServicePrincipalCatalog from './ServicePrincipalCatalog.svelte';

  type IdentityArea = 'review' | 'service-principals' | 'mappings';

  type IdentityAccessReviewWorkspaceProps = {
    readonly state: IdentityReviewLoadState;
    readonly actionState: AdminActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onCreateServicePrincipal?: (request: UpsertServicePrincipalRequest) => boolean | Promise<boolean>;
    readonly onUpdateServicePrincipal?: (id: string, request: UpsertServicePrincipalRequest) => boolean | Promise<boolean>;
    readonly onCreateIdentityMapping?: (request: UpsertIdentityMappingRequest) => boolean | Promise<boolean>;
    readonly onUpdateIdentityMapping?: (id: string, request: UpsertIdentityMappingRequest) => boolean | Promise<boolean>;
  };

  let {
    state: loadState,
    actionState,
    onRefresh,
    onCreateServicePrincipal,
    onUpdateServicePrincipal,
    onCreateIdentityMapping,
    onUpdateIdentityMapping,
  }: IdentityAccessReviewWorkspaceProps = $props();
  let area = $state<IdentityArea>('review');
  const busy = $derived(actionState.status === 'running');
  const review = $derived(loadState.status === 'ready' ? loadState.review : null);
  const model = $derived(review ? buildIdentityReviewViewModel(review) : null);
  const areas = $derived([
    { id: 'review' as const, label: 'Access review', count: review?.projects.length ?? 0, icon: ShieldCheck },
    { id: 'service-principals' as const, label: 'Service principals', count: review?.service_principals.length ?? 0, icon: Bot },
    { id: 'mappings' as const, label: 'Identity mappings', count: review?.identity_mappings.length ?? 0, icon: Waypoints },
  ]);
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="identity-access-workspace">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Govern</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Identity &amp; access</h1>
      <p class="m-0 mt-2 max-w-3xl text-sm leading-6 text-admin-muted">Review sanitized external identity evidence, automation registrations, project mappings, and active delegation correlation.</p>
    </div>
    <button class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60" type="button" onclick={() => void onRefresh?.()} disabled={loadState.status === 'loading' || busy} data-testid="identity-refresh"><RefreshCw size={16} strokeWidth={1.9} /><span>Refresh review</span></button>
  </header>

  <ActionFeedback state={actionState} successTitle="Identity operation completed" errorTitle="Identity operation failed" successTestId="identity-action-success" errorTestId="identity-action-error" runningTestId="identity-action-running" />

  {#if loadState.status === 'loading'}
    <section class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel" data-testid="identity-loading"><AdminMessage tone="loading" title="Loading identity access review" message="Sanitized principal, project, registry, mapping, and delegation evidence is being loaded." /></section>
  {:else if loadState.status === 'error'}
    <AdminMessage tone="error" title="Identity access review unavailable" message={loadState.message} testId="identity-load-error" />
  {:else if review && model}
    <nav class="flex min-w-0 flex-wrap border-b border-admin-border" aria-label="Identity areas">
      {#each areas as definition}
        <button class={`inline-flex h-11 items-center gap-2 border-b-2 px-3 text-sm font-medium ${area === definition.id ? 'border-admin-accent text-admin-ink' : 'border-transparent text-admin-muted hover:text-admin-ink'}`} type="button" onclick={() => { area = definition.id; }} aria-pressed={area === definition.id} data-testid={`identity-area-${definition.id}`}>
          <definition.icon size={15} strokeWidth={1.8} /><span>{definition.label}</span><span class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted">{definition.count}</span>
        </button>
      {/each}
    </nav>

    {#if area === 'review'}
      <IdentityReviewSummary {model} />
    {:else if area === 'service-principals'}
      <ServicePrincipalCatalog {review} {busy} onCreate={onCreateServicePrincipal} onUpdate={onUpdateServicePrincipal} />
    {:else}
      <IdentityMappingCatalog {review} {busy} onCreate={onCreateIdentityMapping} onUpdate={onUpdateIdentityMapping} />
    {/if}
  {/if}
</div>
