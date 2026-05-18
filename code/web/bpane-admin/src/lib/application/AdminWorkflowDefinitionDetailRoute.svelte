<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import type { WorkflowClient } from '../api/workflow-client';
  import type {
    WorkflowDefinitionResource,
    WorkflowDefinitionVersionResource,
    WorkflowRunResource,
  } from '../api/workflow-types';
  import AdminMessage from '../presentation/AdminMessage.svelte';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import {
    WorkflowTemplateCatalogViewModelBuilder,
    type MetadataRow,
  } from '../presentation/workflow-template-catalog-view-model';

  type AdminWorkflowDefinitionDetailRouteProps = {
    readonly workflowClient: WorkflowClient;
    readonly workflowId: string;
  };

  let { workflowClient, workflowId }: AdminWorkflowDefinitionDetailRouteProps = $props();
  let definition = $state<WorkflowDefinitionResource | null>(null);
  let versions = $state<readonly WorkflowDefinitionVersionResource[]>([]);
  let runs = $state<readonly WorkflowRunResource[]>([]);
  let selectedVersion = $state('');
  let loading = $state(false);
  let error = $state<string | null>(null);
  let actionFeedback = $state<AdminMessageFeedback | null>(null);
  let lastRefreshedAt = $state<string | null>(null);

  const viewModel = $derived(definition
    ? WorkflowTemplateCatalogViewModelBuilder.detail({
        definition,
        versions,
        selectedVersion,
        runs,
      })
    : null);

  onMount(() => {
    void loadDetail(false);
  });

  async function loadDetail(showFeedback = true): Promise<void> {
    const previousLatestVersion = definition?.latest_version ?? null;
    const previousRunCount = runs.length;
    loading = true;
    error = null;
    actionFeedback = null;
    try {
      const [definitionResource, versionList, runList] = await Promise.all([
        workflowClient.getDefinition(workflowId),
        workflowClient.listDefinitionVersions(workflowId),
        workflowClient.listRuns(),
      ]);
      definition = definitionResource;
      versions = versionList.versions;
      runs = runList.runs;
      selectedVersion = selectedVersionFor(definitionResource, versionList.versions, selectedVersion);
      lastRefreshedAt = new Date().toISOString();
      if (showFeedback) {
        actionFeedback = successFeedback(refreshMessage(
          previousLatestVersion,
          definitionResource.latest_version,
          previousRunCount,
          runList.runs.length,
        ));
      }
    } catch (loadError) {
      error = errorMessage(loadError);
      actionFeedback = null;
    } finally {
      loading = false;
    }
  }

  function selectedVersionFor(
    nextDefinition: WorkflowDefinitionResource,
    nextVersions: readonly WorkflowDefinitionVersionResource[],
    current: string,
  ): string {
    if (current && nextVersions.some((version) => version.version === current)) {
      return current;
    }
    if (nextDefinition.latest_version) {
      return nextDefinition.latest_version;
    }
    return nextVersions[0]?.version ?? '';
  }

  function runHref(runId: string): string {
    return `${base}/workflow-runs/${encodeURIComponent(runId)}`;
  }

  function formatDate(value: string | null): string {
    return value ? new Date(value).toLocaleString() : 'not refreshed';
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected workflow definition detail error';
  }

  function refreshMessage(
    previousLatestVersion: string | null,
    nextLatestVersion: string | null,
    previousRunCount: number,
    nextRunCount: number,
  ): string {
    if (previousLatestVersion && previousLatestVersion !== nextLatestVersion) {
      return `Latest workflow version changed from ${previousLatestVersion} to ${nextLatestVersion ?? 'none'}.`;
    }
    if (previousRunCount !== nextRunCount) {
      return `Workflow run count changed from ${previousRunCount} to ${nextRunCount}.`;
    }
    return 'Workflow template detail refreshed.';
  }

  function successFeedback(message: string): AdminMessageFeedback {
    return { variant: 'success', title: 'Workflow template refreshed', message, testId: 'workflow-definition-detail-message' };
  }
</script>

<section class="grid gap-5" data-testid="workflow-definition-detail">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div class="min-w-0">
        <p class="admin-eyebrow">Workflow template detail</p>
        <h1
          class="m-0 max-w-full text-xl font-bold text-admin-ink [overflow-wrap:anywhere]"
          data-testid="workflow-definition-detail-title"
        >
          {viewModel?.name ?? workflowId}
        </h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/workflows`}>Workflow catalog</a>
        <a class="admin-button-ghost" href={`${base}/workflow-runs`}>Workflow runs</a>
        <a class="admin-button-ghost" href={`${base}/files/workspaces`}>File workspaces</a>
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="workflow-definition-detail-refresh"
          disabled={loading}
          onclick={() => void loadDetail()}
        >
          Refresh
        </button>
      </div>
    </div>
    <p class="m-0 mt-3 text-sm text-admin-ink/62" data-testid="workflow-definition-detail-last-refresh">
      Last refreshed {formatDate(lastRefreshedAt)}
    </p>
  </div>

  {#if loading && !viewModel}
    <section class="admin-panel mt-0">
      <AdminMessage variant="loading" message="Loading workflow template..." compact={true} />
    </section>
  {:else if error && !viewModel}
    <section class="admin-panel mt-0">
      <AdminMessage variant="error" message={error} testId="workflow-definition-detail-error" compact={true} />
    </section>
  {:else if viewModel}
    {#if error}
      <AdminMessage variant="error" message={error} testId="workflow-definition-detail-action-error" compact={true} />
    {/if}
    {#if actionFeedback}
      <AdminMessage
        variant={actionFeedback.variant}
        title={actionFeedback.title}
        message={actionFeedback.message}
        testId={actionFeedback.testId}
        compact={true}
      />
    {/if}

    <section class="grid gap-3 md:grid-cols-4" aria-label="Workflow definition facts">
      {@render Fact('Kind', viewModel.kind, 'workflow-definition-detail-kind')}
      {@render Fact('Latest', viewModel.latestVersion, 'workflow-definition-detail-latest-version')}
      {@render Fact('Versions', String(viewModel.versionRows.length), 'workflow-definition-detail-version-count')}
      {@render Fact('Runs', String(viewModel.recentRuns.length), 'workflow-definition-detail-run-count')}
    </section>

    <section class="admin-panel mt-0 grid gap-3">
      <div>
        <p class="admin-eyebrow">Definition</p>
        <p class="m-0 text-sm leading-normal text-admin-ink/72">{viewModel.description}</p>
      </div>
      <div class="grid gap-2 text-sm text-admin-ink/72 md:grid-cols-2">
        <span class="[overflow-wrap:anywhere]"><strong>Workflow id:</strong> <code class="admin-code-pill">{viewModel.definitionId}</code></span>
        {#each viewModel.labels as row}
          <span><strong>{row.label}:</strong> {row.value}</span>
        {/each}
      </div>
    </section>

    <section class="grid gap-3 lg:grid-cols-[minmax(220px,320px)_1fr]">
      <div class="admin-panel mt-0 grid self-start gap-2">
        <p class="admin-eyebrow">Versions</p>
        {#if viewModel.versionRows.length === 0}
          <AdminMessage variant="empty" message="No workflow versions are published yet." compact={true} />
        {:else}
          {#each viewModel.versionRows as version}
            <button
              class={`grid min-w-0 gap-1 rounded-xl border p-3 text-left text-sm ${
                selectedVersion === version.version
                  ? 'border-admin-leaf/44 bg-admin-leaf/12 text-admin-ink'
                  : 'border-admin-ink/10 bg-admin-field text-admin-ink/76'
              }`}
              type="button"
              data-testid="workflow-definition-version-row"
              data-version={version.version}
              onclick={() => { selectedVersion = version.version; }}
            >
              <span class="flex min-w-0 items-center justify-between gap-2">
                <strong class="truncate">{version.version}</strong>
                {#if version.latest}
                  <span class="rounded-lg bg-admin-leaf/15 px-2 py-1 text-xs text-admin-leaf">latest</span>
                {/if}
              </span>
              <span class="truncate text-xs">{version.executor} | {version.createdAt}</span>
            </button>
          {/each}
        {/if}
      </div>

      <div class="admin-panel mt-0 grid gap-4">
        <div class="admin-header">
          <div>
            <p class="admin-eyebrow">Version metadata</p>
            <h2 class="admin-section-title" data-testid="workflow-definition-selected-version">
              {viewModel.selectedVersion?.version ?? 'No version selected'}
            </h2>
          </div>
          {#if viewModel.selectedVersion}
            <span class="rounded-xl border border-admin-leaf/25 bg-admin-leaf/10 px-3 py-1 text-xs font-bold text-admin-leaf" data-testid="workflow-definition-version-executor">
              {viewModel.selectedVersion.executor}
            </span>
          {/if}
        </div>

        {#if viewModel.selectedVersion}
          <div class="grid gap-2 text-sm text-admin-ink/72">
            <span class="[overflow-wrap:anywhere]"><strong>Version id:</strong> <code class="admin-code-pill">{viewModel.selectedVersion.id}</code></span>
            <span class="[overflow-wrap:anywhere]" data-testid="workflow-definition-version-entrypoint"><strong>Entrypoint:</strong> {viewModel.selectedVersion.entrypoint}</span>
          </div>
          <div class="grid gap-3 lg:grid-cols-3">
            {@render MetadataPanel('Source', viewModel.selectedVersion.sourceRows, 'workflow-definition-source')}
            {@render MetadataPanel('Policy', viewModel.selectedVersion.policyRows, 'workflow-definition-policy')}
            {@render MetadataPanel('Schemas', viewModel.selectedVersion.schemaRows, 'workflow-definition-schemas')}
          </div>
        {:else}
          <AdminMessage variant="empty" message="No version metadata is available." compact={true} />
        {/if}
      </div>
    </section>

    <section class="admin-panel mt-0 grid gap-3">
      <div class="admin-header">
        <div>
          <p class="admin-eyebrow">Runs</p>
          <h2 class="admin-section-title">Recent runs for this workflow</h2>
        </div>
      </div>
      {#if viewModel.recentRuns.length === 0}
        <AdminMessage variant="empty" message="No workflow runs found for this definition." compact={true} />
      {:else}
        <div class="grid gap-2">
          {#each viewModel.recentRuns as run}
            <a
              class="grid min-w-0 gap-1 rounded-xl border border-admin-ink/10 bg-admin-field p-3 text-sm text-admin-ink no-underline md:grid-cols-[minmax(0,1fr)_auto]"
              href={runHref(run.id)}
              data-testid="workflow-definition-run-link"
            >
              <span class="min-w-0">
                <strong class="block truncate font-mono">{run.id}</strong>
                <span class="text-xs text-admin-ink/58">session {run.sessionId}</span>
              </span>
              <span class="text-xs text-admin-ink/66">v{run.version} | {run.state} | {run.updatedAt}</span>
            </a>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</section>

{#snippet Fact(label: string, value: string, testId: string)}
  <span class="min-w-0 rounded-xl bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
    {label}
    <strong class="mt-1 block truncate font-mono text-admin-ink normal-case" data-testid={testId}>{value}</strong>
  </span>
{/snippet}

{#snippet MetadataPanel(title: string, rows: readonly MetadataRow[], testId: string)}
  <section class="grid min-w-0 gap-2 rounded-xl border border-admin-ink/10 bg-admin-field p-3" data-testid={testId}>
    <h3 class="m-0 text-sm font-extrabold text-admin-ink">{title}</h3>
    {#each rows as row}
      <p class="m-0 min-w-0 text-xs text-admin-ink/68">
        <strong class="block text-admin-ink/84">{row.label}</strong>
        <span class="[overflow-wrap:anywhere]">{row.value}</span>
      </p>
    {/each}
  </section>
{/snippet}
