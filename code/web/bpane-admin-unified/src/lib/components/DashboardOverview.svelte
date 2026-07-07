<script lang="ts">
  import { ArrowUpRight, RefreshCw } from '@lucide/svelte';
  import {
    buildDashboardOverviewModel,
    type DashboardOverviewLoadState,
  } from '$lib/dashboard/dashboard-overview-view-model';
  import type { ProjectTone } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';

  type DashboardOverviewProps = {
    readonly state: DashboardOverviewLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    state: loadState,
    onRefresh,
  }: DashboardOverviewProps = $props();

  const model = $derived(loadState.status === 'ready'
    ? buildDashboardOverviewModel(loadState.snapshot, loadState.failures)
    : null);

  function refresh(): void {
    void onRefresh?.();
  }

  function metricShellClass(tone: ProjectTone): string {
    if (tone === 'success') {
      return 'border-emerald-200 bg-emerald-50/70';
    }
    if (tone === 'warning') {
      return 'border-amber-200 bg-amber-50/70';
    }
    if (tone === 'danger') {
      return 'border-red-200 bg-red-50/70';
    }
    return 'border-admin-border bg-admin-panel';
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="dashboard-overview">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Dashboard</h1>
    </div>

    <button
      class="inline-flex h-10 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink shadow-sm hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loadState.status === 'loading'}
      data-testid="dashboard-refresh"
    >
      <RefreshCw size={16} strokeWidth={1.9} />
      <span>Refresh</span>
    </button>
  </header>

  {#if loadState.status === 'loading'}
    <section
      class="flex min-h-64 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted"
      aria-live="polite"
      data-testid="dashboard-loading"
    >
      <AdminMessage
        tone="loading"
        title="Loading dashboard"
        message="The unified admin catalogs are being refreshed from the control API."
      />
    </section>
  {:else if loadState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Dashboard unavailable"
      message={loadState.message}
      testId="dashboard-error"
    />
  {:else if model}
    {#if loadState.failures.length > 0}
      <AdminMessage
        tone="warning"
        title="Dashboard partially loaded"
        message={`${loadState.failures.length} catalog request${loadState.failures.length === 1 ? '' : 's'} could not be loaded.`}
        items={loadState.failures.slice(0, 4).map((failure) => `${failure.resource}: ${failure.message}`)}
        testId="dashboard-partial-warning"
      />
    {/if}

    <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Dashboard metrics">
      {#each model.metrics as metric}
        <a
          class={`group rounded-md border p-4 shadow-sm transition hover:border-admin-accent hover:bg-white ${metricShellClass(metric.tone)}`}
          href={metric.href}
          data-testid={metric.testId}
        >
          <span class="flex items-center justify-between gap-3">
            <span class="min-w-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</span>
            <ArrowUpRight class="shrink-0 text-admin-muted group-hover:text-admin-accent" size={15} strokeWidth={1.9} />
          </span>
          <span class="mt-2 block text-2xl font-semibold text-admin-ink">{metric.value}</span>
          <span class="mt-1 block text-sm text-admin-muted">{metric.detail}</span>
        </a>
      {/each}
    </section>

    <section class="grid min-w-0 gap-5 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.9fr)]">
      <section class="rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm" data-testid="dashboard-attention-panel">
        <div class="flex items-center justify-between gap-3">
          <div>
            <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Attention</p>
            <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Items to review</h2>
          </div>
          <span class="rounded-full bg-admin-soft px-2.5 py-1 text-xs font-semibold text-admin-muted" data-testid="dashboard-attention-count">
            {model.attentionItems.length}
          </span>
        </div>

        {#if model.attentionItems.length === 0}
          <div class="mt-4 rounded-md border border-dashed border-admin-border bg-admin-soft px-3 py-6 text-center text-sm text-admin-muted" data-testid="dashboard-attention-empty">
            No catalog state currently needs operator attention.
          </div>
        {:else}
          <div class="mt-4 divide-y divide-admin-border" data-testid="dashboard-attention-list">
            {#each model.attentionItems as item}
              <a class="flex min-w-0 items-start gap-3 py-3 hover:bg-admin-soft/70" href={item.href} data-testid={item.testId}>
                <span class={`mt-0.5 rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(item.tone)}`}>
                  {item.tone}
                </span>
                <span class="min-w-0">
                  <span class="block truncate text-sm font-semibold text-admin-ink">{item.title}</span>
                  <span class="mt-0.5 block break-words text-sm text-admin-muted">{item.description}</span>
                </span>
              </a>
            {/each}
          </div>
        {/if}
      </section>

      <section class="rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm" data-testid="dashboard-quick-links">
        <div>
          <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Navigation</p>
          <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Catalog shortcuts</h2>
        </div>

        <div class="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-1">
          {#each model.quickLinks as link}
            <a
              class="group flex min-w-0 items-start justify-between gap-3 rounded-md border border-admin-border bg-white px-3 py-3 shadow-sm hover:border-admin-accent"
              href={link.href}
              data-testid={link.testId}
            >
              <span class="min-w-0">
                <span class="block text-sm font-semibold text-admin-ink">{link.label}</span>
                <span class="mt-0.5 block text-sm text-admin-muted">{link.description}</span>
              </span>
              <span class="flex shrink-0 items-center gap-2">
                <span class="rounded-full bg-admin-soft px-2.5 py-1 text-xs font-semibold text-admin-ink">{link.value}</span>
                <ArrowUpRight class="text-admin-muted group-hover:text-admin-accent" size={15} strokeWidth={1.9} />
              </span>
            </a>
          {/each}
        </div>
      </section>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm" data-testid="dashboard-activity-panel">
      <div>
        <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Activity</p>
        <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Recent operational changes</h2>
      </div>

      {#if model.recentActivity.length === 0}
        <div class="mt-4 rounded-md border border-dashed border-admin-border bg-admin-soft px-3 py-6 text-center text-sm text-admin-muted" data-testid="dashboard-activity-empty">
          No recent sessions, workflow runs, or recordings are visible.
        </div>
      {:else}
        <div class="mt-4 divide-y divide-admin-border" data-testid="dashboard-activity-list">
          {#each model.recentActivity as item}
            <a class="grid gap-2 py-3 text-sm hover:bg-admin-soft/70 md:grid-cols-[minmax(0,1fr)_auto]" href={item.href}>
              <span class="min-w-0">
                <span class="block truncate font-semibold text-admin-ink">{item.title}</span>
                <span class="mt-0.5 block text-admin-muted">{item.detail}</span>
              </span>
              <span class="text-admin-muted">{item.timestamp}</span>
            </a>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>
