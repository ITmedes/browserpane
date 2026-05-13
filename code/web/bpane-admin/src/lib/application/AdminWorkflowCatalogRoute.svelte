<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import type { WorkflowClient } from '../api/workflow-client';
  import type {
    WorkflowDefinitionResource,
    WorkflowDefinitionVersionResource,
  } from '../api/workflow-types';
  import {
    hiddenWorkflowDefinitions,
    includeHiddenWorkflowDefinitions,
    visibleWorkflowDefinitions,
  } from './workflow-definition-visibility';
  import {
    WorkflowTemplateCatalogViewModelBuilder,
    type WorkflowCatalogItem,
  } from '../presentation/workflow-template-catalog-view-model';
  import { WorkflowOperationsService } from './workflow-operations-service';

  type AdminWorkflowCatalogRouteProps = {
    readonly workflowClient: WorkflowClient;
  };

  let { workflowClient }: AdminWorkflowCatalogRouteProps = $props();
  const workflowService = $derived(new WorkflowOperationsService(workflowClient));
  let items = $state<readonly WorkflowCatalogItem[]>([]);
  let hiddenCount = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let search = $state('');
  let lastRefreshedAt = $state<string | null>(null);

  const viewModel = $derived(WorkflowTemplateCatalogViewModelBuilder.catalog({
    items,
    hiddenCount,
    search,
  }));

  onMount(() => {
    void loadCatalog();
  });

  async function loadCatalog(): Promise<void> {
    loading = true;
    error = null;
    try {
      const definitions = (await workflowClient.listDefinitions()).workflows;
      const withTemplate = await workflowService.ensureBrowserPaneTourTemplate(definitions);
      const visible = includeHiddenWorkflowDefinitions()
        ? withTemplate
        : visibleWorkflowDefinitions(withTemplate);
      hiddenCount = includeHiddenWorkflowDefinitions() ? 0 : hiddenWorkflowDefinitions(withTemplate).length;
      items = await loadCatalogItems(visible);
      lastRefreshedAt = new Date().toISOString();
    } catch (loadError) {
      error = errorMessage(loadError);
    } finally {
      loading = false;
    }
  }

  async function loadCatalogItems(
    definitions: readonly WorkflowDefinitionResource[],
  ): Promise<readonly WorkflowCatalogItem[]> {
    return await Promise.all(definitions.map(async (definition) => {
      if (!definition.latest_version) {
        return { definition, latestVersion: null, versionError: null };
      }
      try {
        return {
          definition,
          latestVersion: await workflowClient.getDefinitionVersion(
            definition.id,
            definition.latest_version,
          ),
          versionError: null,
        };
      } catch (versionError) {
        return {
          definition,
          latestVersion: null,
          versionError: errorMessage(versionError),
        };
      }
    }));
  }

  function detailHref(workflowId: string): string {
    return `${base}/workflows/${encodeURIComponent(workflowId)}`;
  }

  function formatDate(value: string | null): string {
    return value ? new Date(value).toLocaleString() : 'not refreshed';
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected workflow catalog error';
  }
</script>

<section class="grid gap-5" data-testid="workflow-catalog">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">Workflow templates</p>
        <h1 class="admin-section-title">Workflow catalog</h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <a class="admin-button-ghost" href={`${base}/workflow-runs`}>Workflow runs</a>
        <a class="admin-button-ghost" href={`${base}/sessions`}>Sessions</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="workflow-catalog-refresh"
          disabled={loading}
          onclick={() => void loadCatalog()}
        >
          Refresh
        </button>
      </div>
    </div>
    <div class="mt-4 grid gap-3 md:grid-cols-[minmax(220px,360px)_1fr] md:items-end">
      <label class="grid gap-1 text-sm font-bold text-admin-ink/72">
        Search
        <input
          class="min-h-11 rounded-xl border border-[#90a6cc]/20 bg-admin-field px-3 text-admin-ink outline-none focus:border-admin-leaf/45"
          data-testid="workflow-catalog-search"
          placeholder="Name, source, executor, label"
          bind:value={search}
        />
      </label>
      <p class="m-0 text-sm text-admin-ink/62" data-testid="workflow-catalog-count">
        {viewModel.rows.length} of {viewModel.totalCount} visible templates | {viewModel.hiddenCount} internal hidden | Last refreshed {formatDate(lastRefreshedAt)}
      </p>
    </div>
  </div>

  {#if loading && items.length === 0}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0">Loading workflow templates...</p>
    </section>
  {:else if error}
    <section class="admin-panel mt-0">
      <p class="admin-error mt-0" data-testid="workflow-catalog-error">{error}</p>
    </section>
  {:else if viewModel.rows.length === 0}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0" data-testid="workflow-catalog-empty">{viewModel.emptyMessage}</p>
    </section>
  {:else}
    <section class="grid gap-2" aria-label="Workflow catalog table">
      {#each viewModel.rows as row}
        <a
          class="grid min-w-0 gap-3 rounded-xl border border-admin-ink/10 bg-admin-panel/82 p-4 text-admin-ink no-underline transition hover:border-admin-leaf/42 hover:bg-admin-field/78 lg:grid-cols-[minmax(0,1fr)_minmax(220px,320px)]"
          href={detailHref(row.id)}
          data-testid="workflow-catalog-row"
          data-workflow-id={row.id}
        >
          <span class="grid min-w-0 gap-1">
            <span class="flex min-w-0 flex-wrap items-center gap-2">
              <strong class="truncate text-base" data-testid="workflow-catalog-row-title" title={row.name}>{row.name}</strong>
              <span class="rounded-lg bg-admin-leaf/12 px-2 py-1 text-xs font-bold text-admin-leaf" data-testid="workflow-catalog-row-kind">{row.kind}</span>
              <span class="rounded-lg bg-admin-field/72 px-2 py-1 text-xs font-bold text-[#c1d0e8]">v{row.latestVersion}</span>
            </span>
            <span class="line-clamp-2 text-sm text-admin-ink/66">{row.description}</span>
            <span class="truncate text-xs text-admin-ink/54">{row.labels}</span>
          </span>
          <span class="grid min-w-0 gap-1 text-xs text-admin-ink/62">
            <span><strong class="text-admin-ink/78">Executor:</strong> {row.executor}</span>
            <span class="truncate" title={row.source}><strong class="text-admin-ink/78">Source:</strong> {row.source}</span>
            <span class="[overflow-wrap:anywhere]" title={row.sourceCommit}><strong class="text-admin-ink/78">Commit:</strong> {row.sourceCommit}</span>
            <span><strong class="text-admin-ink/78">Updated:</strong> {row.updatedAt}</span>
          </span>
        </a>
      {/each}
    </section>
  {/if}
</section>
