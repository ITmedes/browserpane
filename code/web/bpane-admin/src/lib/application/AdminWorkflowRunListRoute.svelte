<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import type { WorkflowClient } from '../api/workflow-client';
  import type { WorkflowRunResource } from '../api/workflow-types';

  type AdminWorkflowRunListRouteProps = {
    readonly workflowClient: WorkflowClient;
  };

  let { workflowClient }: AdminWorkflowRunListRouteProps = $props();
  let runs = $state<readonly WorkflowRunResource[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let search = $state('');
  let lastRefreshedAt = $state<string | null>(null);

  const filteredRuns = $derived(filterRuns(runs, search));

  onMount(() => {
    void loadRuns();
  });

  async function loadRuns(): Promise<void> {
    loading = true;
    error = null;
    try {
      runs = (await workflowClient.listRuns()).runs;
      lastRefreshedAt = new Date().toISOString();
    } catch (loadError) {
      error = errorMessage(loadError);
    } finally {
      loading = false;
    }
  }

  function detailHref(runId: string): string {
    return `${base}/workflow-runs/${encodeURIComponent(runId)}`;
  }

  function sessionHref(sessionId: string): string {
    return `${base}/sessions/${encodeURIComponent(sessionId)}`;
  }

  function filterRuns(items: readonly WorkflowRunResource[], query: string): readonly WorkflowRunResource[] {
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return items;
    }
    return items.filter((run) => [
      run.id,
      run.state,
      run.session_id,
      run.automation_task_id,
      run.workflow_definition_id,
      run.workflow_version,
      run.source_system ?? '',
      run.source_reference ?? '',
      run.client_request_id ?? '',
    ].some((value) => value.toLowerCase().includes(normalized)));
  }

  function formatDate(value: string | null): string {
    return value ? new Date(value).toLocaleString() : 'not refreshed';
  }

  function terminalLabel(run: WorkflowRunResource): string {
    if (run.completed_at) {
      return `completed ${formatDate(run.completed_at)}`;
    }
    if (run.started_at) {
      return `started ${formatDate(run.started_at)}`;
    }
    return `created ${formatDate(run.created_at)}`;
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected workflow run list error';
  }
</script>

<section class="grid gap-5" data-testid="workflow-run-inspector-list">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">Workflow runs</p>
        <h1 class="admin-section-title">Workflow run inspector</h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <a class="admin-button-ghost" href={`${base}/workflows`}>Workflow catalog</a>
        <a class="admin-button-ghost" href={`${base}/sessions`}>Sessions</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="workflow-run-inspector-refresh"
          disabled={loading}
          onclick={() => void loadRuns()}
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
          data-testid="workflow-run-inspector-search"
          placeholder="Run id, state, session, source"
          bind:value={search}
        />
      </label>
      <p class="m-0 text-sm text-admin-ink/62" data-testid="workflow-run-inspector-count">
        {filteredRuns.length} of {runs.length} visible runs | Last refreshed {formatDate(lastRefreshedAt)}
      </p>
    </div>
  </div>

  {#if loading && runs.length === 0}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0">Loading workflow runs...</p>
    </section>
  {:else if error}
    <section class="admin-panel mt-0">
      <p class="admin-error mt-0" data-testid="workflow-run-inspector-error">{error}</p>
    </section>
  {:else if filteredRuns.length === 0}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0" data-testid="workflow-run-inspector-empty">
        No workflow runs match the current filter.
      </p>
    </section>
  {:else}
    <section class="grid gap-2" aria-label="Workflow run table">
      {#each filteredRuns as run}
        <a
          class="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-4 rounded-xl border border-admin-ink/10 bg-admin-panel/82 p-4 text-admin-ink no-underline transition hover:border-admin-leaf/42 hover:bg-admin-field/78"
          href={detailHref(run.id)}
          data-testid="workflow-run-inspector-row"
          data-run-id={run.id}
        >
          <span class="grid min-w-0 gap-1">
            <strong class="truncate font-mono text-sm" title={run.id}>{run.id}</strong>
            <span class="truncate text-xs text-admin-ink/58">
              v{run.workflow_version} | {terminalLabel(run)}
            </span>
            <span class="truncate text-xs text-admin-ink/58">
              session <span class="font-mono">{run.session_id}</span>
            </span>
          </span>
          <span class="grid justify-items-end gap-1 text-xs text-[#c1d0e8]">
            <span class="rounded-lg bg-admin-field/72 px-2 py-1" data-testid="workflow-run-inspector-row-state">
              {run.state}
            </span>
            <span class="rounded-lg bg-admin-field/72 px-2 py-1">
              {run.produced_files.length} files
            </span>
          </span>
        </a>
        <a class="sr-only" href={sessionHref(run.session_id)}>Linked session</a>
      {/each}
    </section>
  {/if}
</section>
