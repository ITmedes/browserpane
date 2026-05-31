<script lang="ts">
  import { RefreshCw } from 'lucide-svelte';
  import type { IdentityAccessReviewResponse } from '../api/control-types';
  import AdminMessage from './AdminMessage.svelte';
  import { IdentityAccessReviewViewModelBuilder } from './identity-access-review-view-model';

  type IdentityAccessReviewPanelProps = {
    readonly review: IdentityAccessReviewResponse | null;
    readonly loading?: boolean;
    readonly error?: string | null;
    readonly onRefresh: () => void;
  };

  let {
    review,
    loading = false,
    error = null,
    onRefresh,
  }: IdentityAccessReviewPanelProps = $props();

  const viewModel = $derived(review ? IdentityAccessReviewViewModelBuilder.build(review) : null);
</script>

<section class="grid min-w-0 gap-4" aria-label="Identity access review" data-testid="identity-access-review-panel">
  <div class="flex min-w-0 flex-wrap items-center justify-between gap-2">
    <p class="m-0 text-sm leading-normal text-admin-ink/68">
      Review the authenticated principal, owner-scoped resource counts, registered service principals, and delegated automation access.
    </p>
    <button
      class="admin-button-ghost inline-flex items-center gap-2"
      type="button"
      data-testid="identity-refresh"
      disabled={loading}
      onclick={onRefresh}
    >
      <RefreshCw size={15} class={loading ? 'animate-spin' : ''} aria-hidden="true" />
      Refresh
    </button>
  </div>

  {#if error}
    <AdminMessage
      variant="warning"
      title="Identity review unavailable"
      message={error}
      testId="identity-access-review-error"
      compact={true}
    />
  {/if}

  {#if loading && !viewModel}
    <AdminMessage
      variant="loading"
      title="Loading identity review"
      message="Fetching the current principal and access summary."
      testId="identity-access-review-loading"
      compact={true}
    />
  {:else if !viewModel}
    <AdminMessage
      variant="empty"
      title="No identity review loaded"
      message="Refresh the panel to load the current access review."
      testId="identity-access-review-empty"
      compact={true}
    />
  {:else}
    <section class="rounded-[14px] border border-[#90a6cc]/18 bg-[#111e32]/72 p-3" aria-label="Current principal">
      <div class="grid min-w-0 gap-2">
        <p class="admin-eyebrow m-0">Current principal</p>
        <div class="grid min-w-0 gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start">
          <div class="min-w-0">
            <strong class="block truncate text-base font-extrabold text-admin-ink" data-testid="identity-principal-title">
              {viewModel.principalTitle}
            </strong>
            <p class="m-0 min-w-0 text-xs font-semibold leading-normal text-[#c1d0e8] [overflow-wrap:anywhere]" data-testid="identity-principal-subtitle">
              {viewModel.principalSubtitle}
            </p>
          </div>
          <span class="w-fit rounded-xl border border-admin-leaf/24 bg-admin-leaf/10 px-3 py-1 text-xs font-bold text-admin-leaf" data-testid="identity-principal-type">
            {viewModel.principalTypeLabel}
          </span>
        </div>
        <p class="m-0 text-xs font-semibold text-admin-ink/58" data-testid="identity-review-generated-at">
          Generated {viewModel.generatedAtLabel}
        </p>
      </div>
    </section>

    <div class="grid grid-cols-2 gap-2 max-[760px]:grid-cols-1" data-testid="identity-resource-counts">
      {#each viewModel.metrics as metric (metric.key)}
        <span class="rounded-[12px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/62 uppercase">
          {metric.label}
          <strong class="mt-1 block text-admin-ink normal-case" data-testid={metric.testId}>{metric.value}</strong>
        </span>
      {/each}
    </div>

    <section class="grid min-w-0 gap-2" aria-label="Project access" data-testid="identity-project-list">
      <div class="flex items-center justify-between gap-2">
        <p class="admin-eyebrow m-0">Projects</p>
        <span class="text-xs font-bold text-admin-ink/58">{viewModel.projects.length}</span>
      </div>
      {#if viewModel.projects.length === 0}
        <AdminMessage variant="empty" message="No projects are visible for this principal." compact={true} />
      {:else}
        <div class="grid gap-2">
          {#each viewModel.projects as project (project.id)}
            <article class="rounded-[12px] border border-[#90a6cc]/16 bg-admin-panel/62 p-3">
              <div class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start">
                <div class="min-w-0">
                  <strong class="block truncate text-sm font-extrabold text-admin-ink">{project.name}</strong>
                  <p class="m-0 truncate text-xs font-semibold text-admin-ink/54">{project.id}</p>
                </div>
                <span class="w-fit rounded-lg bg-admin-cream/72 px-2 py-1 text-xs font-bold text-admin-ink/68">{project.state}</span>
              </div>
              <div class="mt-3 grid grid-cols-3 gap-2 max-[760px]:grid-cols-1">
                <span class="text-xs font-bold text-admin-ink/58">Sessions <strong class="block text-admin-ink">{project.activeSessions}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Workflows <strong class="block text-admin-ink">{project.activeWorkflowRuns}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Storage <strong class="block text-admin-ink">{project.retainedStorage}</strong></span>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    </section>

    <section class="grid min-w-0 gap-2" aria-label="Service principal registry" data-testid="identity-service-principal-list">
      <div class="flex items-center justify-between gap-2">
        <p class="admin-eyebrow m-0">Service principals</p>
        <span class="text-xs font-bold text-admin-ink/58">{viewModel.servicePrincipals.length}</span>
      </div>
      {#if viewModel.servicePrincipals.length === 0}
        <AdminMessage variant="empty" message="No service principals are registered for this principal." compact={true} />
      {:else}
        <div class="grid gap-2">
          {#each viewModel.servicePrincipals as servicePrincipal (servicePrincipal.id)}
            <article class="rounded-[12px] border border-[#90a6cc]/16 bg-admin-panel/62 p-3">
              <div class="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-start">
                <div class="min-w-0">
                  <strong class="block truncate text-sm font-extrabold text-admin-ink">{servicePrincipal.name}</strong>
                  <p class="m-0 min-w-0 text-xs font-semibold leading-normal text-admin-ink/58 [overflow-wrap:anywhere]">
                    {servicePrincipal.clientId} / {servicePrincipal.issuer}
                  </p>
                </div>
                <span class={`w-fit rounded-lg px-2 py-1 text-xs font-bold ${
                  servicePrincipal.state === 'active'
                    ? 'bg-admin-leaf/12 text-admin-leaf'
                    : 'bg-admin-danger/12 text-admin-danger'
                }`}>
                  {servicePrincipal.state}
                </span>
              </div>
              <div class="mt-3 grid grid-cols-3 gap-2 max-[760px]:grid-cols-1">
                <span class="text-xs font-bold text-admin-ink/58">Delegated <strong class="block text-admin-ink">{servicePrincipal.delegatedSummary}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Scopes <strong class="block text-admin-ink">{servicePrincipal.scopes}</strong></span>
                <span class="text-xs font-bold text-admin-ink/58">Projects <strong class="block text-admin-ink">{servicePrincipal.projects}</strong></span>
              </div>
              <p class="mt-2 mb-0 text-xs font-semibold text-admin-ink/54">
                {servicePrincipal.lastActivity}
                <span class="block [overflow-wrap:anywhere]">{servicePrincipal.delegatedSessionIds}</span>
              </p>
            </article>
          {/each}
        </div>
      {/if}
    </section>

    <section class="grid min-w-0 gap-2" aria-label="Delegated automation access" data-testid="identity-delegation-list">
      <div class="flex items-center justify-between gap-2">
        <p class="admin-eyebrow m-0">Delegated automation</p>
        <span class="text-xs font-bold text-admin-ink/58">{viewModel.delegations.length}</span>
      </div>
      {#if viewModel.delegations.length === 0}
        <AdminMessage variant="empty" message="No delegated automation principals are assigned to visible sessions." compact={true} />
      {:else}
        <div class="grid gap-2">
          {#each viewModel.delegations as delegation (delegation.clientId + delegation.issuer)}
            <article class="rounded-[12px] border border-[#90a6cc]/16 bg-admin-panel/62 p-3">
              <strong class="block truncate text-sm font-extrabold text-admin-ink">{delegation.displayName}</strong>
              <p class="m-0 min-w-0 text-xs font-semibold leading-normal text-admin-ink/58 [overflow-wrap:anywhere]">
                {delegation.clientId} / {delegation.issuer}
              </p>
              <p class="mt-2 mb-0 text-xs font-bold text-admin-ink/72">
                {delegation.sessionSummary} / {delegation.registration} / {delegation.state}
                <span class="block font-semibold text-admin-ink/54">{delegation.sessionIds}</span>
              </p>
            </article>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</section>
