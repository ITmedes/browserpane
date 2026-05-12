<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import type { WorkflowClient } from '../api/workflow-client';
  import type {
    WorkflowRunEventResource,
    WorkflowRunLogResource,
    WorkflowRunProducedFileResource,
    WorkflowRunResource,
  } from '../api/workflow-types';

  type AdminWorkflowRunDetailRouteProps = {
    readonly workflowClient: WorkflowClient;
    readonly runId: string;
  };

  let { workflowClient, runId }: AdminWorkflowRunDetailRouteProps = $props();
  let run = $state<WorkflowRunResource | null>(null);
  let events = $state<readonly WorkflowRunEventResource[]>([]);
  let logs = $state<readonly WorkflowRunLogResource[]>([]);
  let files = $state<readonly WorkflowRunProducedFileResource[]>([]);
  let loading = $state(false);
  let actionInFlight = $state(false);
  let error = $state<string | null>(null);
  let evidenceError = $state<string | null>(null);
  let lastRefreshedAt = $state<string | null>(null);
  let operatorInputText = $state('{}');
  let rejectReason = $state('Rejected from admin');

  const terminal = $derived(isTerminal(run?.state ?? null));
  const pendingRequest = $derived(run?.intervention.pending_request ?? null);
  const operatorInputValid = $derived(isJson(operatorInputText));

  onMount(() => {
    void refreshInspector();
  });

  async function refreshInspector(): Promise<void> {
    loading = true;
    error = null;
    evidenceError = null;
    try {
      run = await workflowClient.getRun(runId);
      await loadEvidence();
      lastRefreshedAt = new Date().toISOString();
    } catch (refreshError) {
      error = errorMessage(refreshError, 'Unexpected workflow run detail error');
    } finally {
      loading = false;
    }
  }

  async function loadEvidence(): Promise<void> {
    const [eventResult, logResult, fileResult] = await Promise.allSettled([
      workflowClient.listRunEvents(runId),
      workflowClient.listRunLogs(runId),
      workflowClient.listProducedFiles(runId),
    ]);
    const errors: string[] = [];
    if (eventResult.status === 'fulfilled') {
      events = eventResult.value.events;
    } else {
      events = [];
      errors.push(errorMessage(eventResult.reason, 'Workflow run events failed'));
    }
    if (logResult.status === 'fulfilled') {
      logs = logResult.value.logs;
    } else {
      logs = [];
      errors.push(errorMessage(logResult.reason, 'Workflow run logs failed'));
    }
    if (fileResult.status === 'fulfilled') {
      files = fileResult.value.files;
    } else {
      files = [];
      errors.push(errorMessage(fileResult.reason, 'Workflow run produced files failed'));
    }
    evidenceError = errors.length > 0 ? errors.join(' | ') : null;
  }

  async function mutateRun(action: () => Promise<WorkflowRunResource>): Promise<void> {
    actionInFlight = true;
    error = null;
    try {
      run = await action();
      await loadEvidence();
      lastRefreshedAt = new Date().toISOString();
    } catch (mutationError) {
      error = errorMessage(mutationError, 'Unexpected workflow run action error');
    } finally {
      actionInFlight = false;
    }
  }

  async function downloadProducedFile(fileId: string): Promise<void> {
    const file = files.find((entry) => entry.file_id === fileId);
    if (!file) {
      return;
    }
    try {
      const url = URL.createObjectURL(await workflowClient.downloadProducedFileContent(file));
      const link = document.createElement('a');
      link.href = url;
      link.download = file.file_name;
      document.body.append(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
    } catch (downloadError) {
      error = errorMessage(downloadError, 'Produced file download failed');
    }
  }

  function submitInput(): void {
    if (!operatorInputValid) {
      error = 'Operator input must be valid JSON.';
      return;
    }
    void mutateRun(() => workflowClient.submitRunInput(runId, {
      input: parseJson(operatorInputText),
      comment: 'operator input from workflow run detail',
    }));
  }

  function formatDate(value: string | null | undefined): string {
    return value ? new Date(value).toLocaleString() : '--';
  }

  function formatJson(value: unknown): string {
    if (value === undefined || value === null) {
      return '--';
    }
    const serialized = JSON.stringify(value, null, 2);
    return serialized.length > 900 ? `${serialized.slice(0, 900)}...` : serialized;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) {
      return `${bytes} B`;
    }
    if (bytes < 1024 * 1024) {
      return `${(bytes / 1024).toFixed(1)} KiB`;
    }
    return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
  }

  function parseJson(text: string): unknown {
    return JSON.parse(text.trim() || '{}');
  }

  function isJson(text: string): boolean {
    try {
      parseJson(text);
      return true;
    } catch {
      return false;
    }
  }

  function isTerminal(state: string | null): boolean {
    return state === 'succeeded' || state === 'failed' || state === 'cancelled' || state === 'timed_out';
  }

  function errorMessage(value: unknown, fallback: string): string {
    return value instanceof Error ? value.message : fallback;
  }
</script>

<section class="grid gap-5" data-testid="workflow-run-inspector-detail">
  <div class="admin-panel mt-0">
    <div class="admin-header">
      <div class="min-w-0">
        <p class="admin-eyebrow">Workflow run detail</p>
        <h1 class="m-0 truncate font-mono text-xl font-bold text-admin-ink" data-testid="workflow-run-inspector-title">
          {run?.id ?? runId}
        </h1>
      </div>
      <div class="admin-actions">
        <a class="admin-button-ghost" href={`${base}/workflow-runs`}>Workflow runs</a>
        {#if run}
          <a class="admin-button-ghost" data-testid="workflow-run-session-link" href={`${base}/sessions/${encodeURIComponent(run.session_id)}`}>Session</a>
        {/if}
        <a class="admin-button-ghost" href={`${base}/`}>Live workspace</a>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="workflow-run-inspector-detail-refresh"
          disabled={loading || actionInFlight}
          onclick={() => void refreshInspector()}
        >
          Refresh
        </button>
      </div>
    </div>
    <p class="m-0 mt-3 text-sm text-admin-ink/62" data-testid="workflow-run-inspector-last-refresh">
      Last refreshed {formatDate(lastRefreshedAt)}
    </p>
  </div>

  {#if loading && !run}
    <section class="admin-panel mt-0">
      <p class="admin-empty mt-0">Loading workflow run...</p>
    </section>
  {:else if error && !run}
    <section class="admin-panel mt-0">
      <p class="admin-error mt-0" data-testid="workflow-run-inspector-detail-error">{error}</p>
    </section>
  {:else if run}
    {#if error}
      <p class="admin-error" data-testid="workflow-run-inspector-action-error">{error}</p>
    {/if}

    <section class="grid gap-3 md:grid-cols-4" aria-label="Workflow run facts">
      {@render Fact('State', run.state, 'workflow-run-detail-state')}
      {@render Fact('Version', run.workflow_version, 'workflow-run-detail-version')}
      {@render Fact('Logs', String(logs.length), 'workflow-run-detail-log-count')}
      {@render Fact('Files', String(files.length), 'workflow-run-detail-file-count')}
    </section>

    <section class="admin-panel mt-0 grid gap-4">
      <div class="grid gap-3 lg:grid-cols-2">
        <div class="grid min-w-0 gap-2 text-sm text-admin-ink/72">
          <span><strong>Workflow:</strong> <code class="admin-code-pill">{run.workflow_definition_id}</code></span>
          <span><strong>Version id:</strong> <code class="admin-code-pill">{run.workflow_definition_version_id}</code></span>
          <span><strong>Session:</strong> <code class="admin-code-pill" data-testid="workflow-run-detail-session-id">{run.session_id}</code></span>
          <span><strong>Automation task:</strong> <code class="admin-code-pill">{run.automation_task_id}</code></span>
          <span><strong>Source:</strong> {run.source_system ?? '--'} {run.source_reference ?? ''}</span>
          <span><strong>Client request:</strong> {run.client_request_id ?? '--'}</span>
        </div>
        <div class="grid min-w-0 gap-2 text-sm text-admin-ink/72">
          <span><strong>Created:</strong> {formatDate(run.created_at)}</span>
          <span><strong>Updated:</strong> {formatDate(run.updated_at)}</span>
          <span><strong>Started:</strong> {formatDate(run.started_at)}</span>
          <span><strong>Completed:</strong> {formatDate(run.completed_at)}</span>
          <span><strong>Runtime:</strong> {run.runtime?.resume_mode ?? '--'} | exact {run.runtime?.exact_runtime_available ? 'yes' : 'no'}</span>
          <span><strong>Hold until:</strong> {formatDate(run.runtime?.hold_until)}</span>
        </div>
      </div>
      {#if run.error}
        <p class="admin-error m-0" data-testid="workflow-run-detail-terminal-error">{run.error}</p>
      {/if}
    </section>

    <section class="admin-panel mt-0 grid gap-4">
      <div class="admin-header">
        <div>
          <p class="admin-eyebrow">Controls</p>
          <h2 class="admin-section-title">Run actions</h2>
        </div>
        <div class="admin-actions">
          <button
            class="admin-button-primary"
            type="button"
            data-testid="workflow-run-detail-cancel"
            disabled={terminal || actionInFlight}
            onclick={() => void mutateRun(() => workflowClient.cancelRun(runId))}
          >
            Cancel
          </button>
          <button
            class="admin-button-primary"
            type="button"
            data-testid="workflow-run-detail-resume"
            disabled={!pendingRequest || actionInFlight}
            onclick={() => void mutateRun(() => workflowClient.resumeRun(runId, { comment: 'released from workflow run detail' }))}
          >
            Resume
          </button>
        </div>
      </div>

      <p class="m-0 text-sm text-admin-ink/70" data-testid="workflow-run-detail-pending-prompt">
        {pendingRequest?.prompt ?? pendingRequest?.kind ?? 'No pending operator input.'}
      </p>
      <label class="grid gap-1.5 text-sm font-bold text-admin-ink">
        Operator input JSON
        <textarea
          class="min-h-24 rounded-xl border border-admin-ink/14 bg-admin-field p-3 font-mono text-xs text-admin-ink"
          data-testid="workflow-run-detail-operator-input"
          bind:value={operatorInputText}
          disabled={!pendingRequest || actionInFlight}
        ></textarea>
      </label>
      <label class="grid gap-1.5 text-sm font-bold text-admin-ink">
        Reject reason
        <input
          class="min-h-10 rounded-xl border border-admin-ink/14 bg-admin-field px-3 text-admin-ink"
          data-testid="workflow-run-detail-reject-reason"
          bind:value={rejectReason}
          disabled={!pendingRequest || actionInFlight}
        />
      </label>
      <div class="flex flex-wrap gap-2">
        <button
          class="admin-button-primary"
          type="button"
          data-testid="workflow-run-detail-submit-input"
          disabled={!pendingRequest || !operatorInputValid || actionInFlight}
          onclick={submitInput}
        >
          Submit input
        </button>
        <button
          class="admin-button-primary"
          type="button"
          data-testid="workflow-run-detail-reject"
          disabled={!pendingRequest || rejectReason.trim().length === 0 || actionInFlight}
          onclick={() => void mutateRun(() => workflowClient.rejectRun(runId, { reason: rejectReason.trim() }))}
        >
          Reject
        </button>
      </div>
    </section>

    <section class="grid gap-3 lg:grid-cols-2">
      {@render JsonPanel('Input', run.input, 'workflow-run-detail-input')}
      {@render JsonPanel('Output', run.output, 'workflow-run-detail-output')}
    </section>

    <section class="grid gap-3 lg:grid-cols-2">
      {@render Timeline('Recent events', events, 'No workflow events loaded.', 'workflow-run-detail-event-count')}
      {@render LogTimeline('Recent logs', logs, 'No workflow logs loaded.', 'workflow-run-detail-log-list-count')}
    </section>

    <section class="admin-panel mt-0 grid gap-3">
      <div class="admin-header">
        <div>
          <p class="admin-eyebrow">Produced files</p>
          <h2 class="admin-section-title">Artifacts</h2>
        </div>
        <span class="text-sm font-bold text-admin-ink/70" data-testid="workflow-run-detail-produced-file-count">
          {files.length} files
        </span>
      </div>
      {#if files.length === 0}
        <p class="admin-empty mt-0">No produced files loaded.</p>
      {:else}
        <div class="grid gap-2">
          {#each files as file}
            <button
              class="flex min-w-0 items-center justify-between gap-3 rounded-xl border border-admin-ink/10 bg-admin-field p-3 text-left text-sm text-admin-ink"
              type="button"
              data-testid="workflow-run-detail-produced-file"
              onclick={() => void downloadProducedFile(file.file_id)}
            >
              <span class="min-w-0 truncate font-bold">{file.file_name}</span>
              <span class="shrink-0 text-xs text-admin-ink/58">{formatBytes(file.byte_count)}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#if evidenceError}
        <p class="admin-error" data-testid="workflow-run-inspector-evidence-error">{evidenceError}</p>
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

{#snippet JsonPanel(title: string, value: unknown, testId: string)}
  <section class="admin-panel mt-0">
    <p class="admin-eyebrow">{title}</p>
    <pre class="m-0 max-h-[320px] overflow-auto rounded-xl bg-admin-night/80 p-3 text-xs text-admin-ink/78" data-testid={testId}>{formatJson(value)}</pre>
  </section>
{/snippet}

{#snippet Timeline(title: string, rows: readonly WorkflowRunEventResource[], empty: string, testId: string)}
  <section class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">{title}</p>
        <h2 class="admin-section-title" data-testid={testId}>{rows.length}</h2>
      </div>
    </div>
    {#if rows.length === 0}
      <p class="admin-empty mt-0">{empty}</p>
    {:else}
      <div class="mt-3 grid gap-2">
        {#each rows.slice(-8).reverse() as row}
          <p class="m-0 rounded-xl bg-admin-field p-3 text-xs text-admin-ink/70">
            <strong class="block truncate text-admin-ink">{row.event_type}</strong>
            <span class="[overflow-wrap:anywhere]">{row.message}</span>
          </p>
        {/each}
      </div>
    {/if}
  </section>
{/snippet}

{#snippet LogTimeline(title: string, rows: readonly WorkflowRunLogResource[], empty: string, testId: string)}
  <section class="admin-panel mt-0">
    <div class="admin-header">
      <div>
        <p class="admin-eyebrow">{title}</p>
        <h2 class="admin-section-title" data-testid={testId}>{rows.length}</h2>
      </div>
    </div>
    {#if rows.length === 0}
      <p class="admin-empty mt-0">{empty}</p>
    {:else}
      <div class="mt-3 grid gap-2">
        {#each rows.slice(-8).reverse() as row}
          <p class="m-0 rounded-xl bg-admin-field p-3 text-xs text-admin-ink/70">
            <strong class="block truncate text-admin-ink">{row.source} {row.stream}</strong>
            <span class="[overflow-wrap:anywhere]">{row.message}</span>
          </p>
        {/each}
      </div>
    {/if}
  </section>
{/snippet}
