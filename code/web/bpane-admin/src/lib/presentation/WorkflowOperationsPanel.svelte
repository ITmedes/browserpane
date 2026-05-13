<script lang="ts">
  import { base } from '$app/paths';
  import { Download, Play, Plug, RefreshCw, Send, Unlock, XCircle } from 'lucide-svelte';
  import type { WorkflowOperationsViewModel } from './workflow-operations-view-model';

  type WorkflowOperationsPanelProps = {
    readonly viewModel: WorkflowOperationsViewModel;
    readonly inputText: string;
    readonly interventionInputText: string;
    readonly onRefreshDefinitions: () => void;
    readonly onWorkflowChange: (workflowId: string) => void;
    readonly onVersionChange: (version: string) => void;
    readonly onInputTextChange: (value: string) => void;
    readonly onInterventionInputChange: (value: string) => void;
    readonly onCreateSession: () => void;
    readonly onConnectSession: () => void;
    readonly onInvokeRun: () => void;
    readonly onRefreshRun: () => void;
    readonly onCancelRun: () => void;
    readonly onReleaseHold: () => void;
    readonly onSubmitInput: () => void;
    readonly onDownloadProducedFile: (fileId: string) => void;
  };

  let props: WorkflowOperationsPanelProps = $props();

  function runDetailHref(runId: string): string {
    return `${base}/workflow-runs/${encodeURIComponent(runId)}`;
  }

  function workflowDetailHref(workflowId: string): string {
    return `${base}/workflows/${encodeURIComponent(workflowId)}`;
  }
</script>

<section class="grid min-w-0 gap-4" aria-label="Workflow operations">
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="min-w-0">
      <p class="admin-eyebrow">Workflows</p>
      <h2 class="m-0 text-base font-extrabold text-admin-ink">{props.viewModel.title}</h2>
    </div>
    <span class="rounded-full bg-admin-warm/12 px-3 py-1 text-xs font-extrabold text-admin-warm" data-testid="workflow-status">
      {props.viewModel.status}
    </span>
  </div>

  <p class="m-0 text-sm leading-normal text-admin-ink/68">{props.viewModel.note}</p>
  {#if props.viewModel.error}
    <p class="admin-error" data-testid="workflow-error">{props.viewModel.error}</p>
  {/if}

  <div class="grid min-w-0 gap-3 rounded-[16px] bg-admin-cream/70 p-3">
    <div class="grid min-w-0 gap-3 md:grid-cols-[minmax(0,1fr)_minmax(8rem,12rem)]">
      <label class="grid min-w-0 gap-1.5 text-sm font-bold text-admin-ink">
        Template
        <select
          class="min-h-10 min-w-0 rounded-xl border border-admin-ink/14 bg-admin-cream px-3 text-admin-ink"
          data-testid="workflow-definition-select"
          value={props.viewModel.selectedWorkflowId}
          disabled={props.viewModel.loading}
          onchange={(event) => props.onWorkflowChange(event.currentTarget.value)}
        >
          {#if props.viewModel.definitionOptions.length === 0}
            <option value="">No workflow templates</option>
          {:else}
            {#each props.viewModel.definitionOptions as definition}
              <option value={definition.id}>{definition.label}</option>
            {/each}
          {/if}
        </select>
      </label>
      <label class="grid min-w-0 gap-1.5 text-sm font-bold text-admin-ink">
        Version
        <input
          class="min-h-10 min-w-0 rounded-xl border border-admin-ink/14 bg-admin-cream px-3 text-admin-ink"
          data-testid="workflow-version"
          value={props.viewModel.selectedVersion}
          disabled={props.viewModel.loading}
          oninput={(event) => props.onVersionChange(event.currentTarget.value)}
        />
      </label>
    </div>

    <label class="grid min-w-0 gap-1.5 text-sm font-bold text-admin-ink">
      Run input JSON
      <textarea
        class="min-h-24 min-w-0 rounded-xl border border-admin-ink/14 bg-admin-cream p-3 font-mono text-xs text-admin-ink"
        data-testid="workflow-input"
        value={props.inputText}
        disabled={props.viewModel.loading}
        oninput={(event) => props.onInputTextChange(event.currentTarget.value)}
      ></textarea>
    </label>

    <div class="flex min-w-0 flex-wrap gap-2">
      <button class="admin-button-primary" type="button" data-testid="workflow-refresh" disabled={!props.viewModel.canRefresh} onclick={props.onRefreshDefinitions}>
        <RefreshCw size={14} aria-hidden="true" /> Refresh
      </button>
      <a class="admin-button-ghost" data-testid="workflow-catalog-link" href={`${base}/workflows`}>
        Catalog
      </a>
      {#if props.viewModel.selectedWorkflowId}
        <a class="admin-button-ghost" data-testid="workflow-definition-detail-link" href={workflowDetailHref(props.viewModel.selectedWorkflowId)}>
          Template detail
        </a>
      {/if}
      {#if props.viewModel.canCreateBaseline}
        <button class="admin-button-primary" type="button" data-testid="workflow-create-session" onclick={props.onCreateSession}>
          <Plug size={14} aria-hidden="true" /> Create session
        </button>
      {/if}
      {#if props.viewModel.canConnectBaseline}
        <button class="admin-button-primary" type="button" data-testid="workflow-connect-session" onclick={props.onConnectSession}>
          <Plug size={14} aria-hidden="true" /> Connect session
        </button>
      {/if}
      <button
        class="admin-button-primary"
        type="button"
        data-testid="workflow-invoke"
        disabled={!props.viewModel.canRun}
        title={props.viewModel.invokeBlockedReason ?? 'Invoke workflow run'}
        aria-describedby={props.viewModel.invokeBlockedReason ? 'workflow-invoke-disabled-reason' : undefined}
        onclick={props.onInvokeRun}
      >
        <Play size={14} aria-hidden="true" /> Invoke run
      </button>
      <button class="admin-button-primary" type="button" data-testid="workflow-run-refresh" disabled={!props.viewModel.canRefreshRun} onclick={props.onRefreshRun}>
        <RefreshCw size={14} aria-hidden="true" /> Refresh run
      </button>
      <button class="admin-button-primary" type="button" data-testid="workflow-cancel" disabled={!props.viewModel.canCancel} onclick={props.onCancelRun}>
        <XCircle size={14} aria-hidden="true" /> Cancel
      </button>
      {#if props.viewModel.currentRunId !== '--'}
        <a class="admin-button-ghost" data-testid="workflow-run-detail-link" href={runDetailHref(props.viewModel.currentRunId)}>
          Inspect details
        </a>
      {/if}
    </div>
    {#if props.viewModel.invokeBlockedReason}
      <p
        class="m-0 rounded-xl border border-admin-warm/20 bg-admin-warm/10 px-3 py-2 text-sm font-bold text-admin-ink"
        id="workflow-invoke-disabled-reason"
        data-testid="workflow-invoke-disabled-reason"
      >
        {props.viewModel.invokeBlockedReason}
      </p>
    {/if}
  </div>

  <div class="grid min-w-0 grid-cols-2 gap-2 xl:grid-cols-4 max-[640px]:grid-cols-1">
    {@render Metric('Baseline', props.viewModel.selectedSessionLabel, 'workflow-session-id')}
    {@render Metric('Run session', props.viewModel.runSessionLabel, 'workflow-run-session-id')}
    {@render Metric('Run', props.viewModel.currentRunState, 'workflow-run-state')}
    {@render Metric('Logs', String(props.viewModel.logCount), 'workflow-log-count')}
    {@render Metric('Artifacts', String(props.viewModel.fileCount), 'workflow-produced-file-count')}
  </div>

  <div class="grid min-w-0 gap-3 rounded-[16px] border border-admin-ink/10 bg-admin-cream/55 p-3">
    <div class="grid min-w-0 gap-1 text-sm text-admin-ink/70">
      <span><strong>Executor:</strong> {props.viewModel.executorLabel}</span>
      <span data-testid="workflow-run-session-note"><strong>Session:</strong> {props.viewModel.runSessionNote}</span>
      <span class="[overflow-wrap:anywhere]" data-testid="workflow-run-id"><strong>Run id:</strong> {props.viewModel.currentRunId}</span>
      <span data-testid="workflow-event-count"><strong>Events:</strong> {props.viewModel.eventCount}</span>
    </div>

    <label class="grid min-w-0 gap-1.5 text-sm font-bold text-admin-ink">
      Operator input JSON
      <textarea
        class="min-h-20 min-w-0 rounded-xl border border-admin-ink/14 bg-admin-cream p-3 font-mono text-xs text-admin-ink"
        data-testid="workflow-intervention-input"
        value={props.interventionInputText}
        disabled={!props.viewModel.canSubmitInput}
        oninput={(event) => props.onInterventionInputChange(event.currentTarget.value)}
      ></textarea>
    </label>
    <p class="m-0 text-sm text-admin-ink/68" data-testid="workflow-pending-prompt">{props.viewModel.pendingPrompt}</p>
    <div class="flex flex-wrap gap-2">
      <button class="admin-button-primary" type="button" data-testid="workflow-submit-input" disabled={!props.viewModel.canSubmitInput} onclick={props.onSubmitInput}>
        <Send size={14} aria-hidden="true" /> Submit input
      </button>
      <button class="admin-button-primary" type="button" data-testid="workflow-release-hold" disabled={!props.viewModel.canReleaseHold} onclick={props.onReleaseHold}>
        <Unlock size={14} aria-hidden="true" /> Release hold
      </button>
    </div>
  </div>

  <div class="grid min-w-0 gap-3 lg:grid-cols-2">
    {@render Timeline('Recent events', props.viewModel.recentEvents, 'No workflow events loaded.')}
    {@render Timeline('Recent logs', props.viewModel.recentLogs, 'No workflow logs loaded.')}
  </div>

  <div class="grid min-w-0 gap-2">
    <h3 class="m-0 text-sm font-extrabold text-admin-ink">Produced files</h3>
    {#if props.viewModel.producedFiles.length === 0}
      <p class="admin-empty">No produced artifacts loaded.</p>
    {:else}
      <div class="grid min-w-0 gap-2">
        {#each props.viewModel.producedFiles as file}
          <button class="flex min-w-0 items-center justify-between gap-3 rounded-xl border border-admin-ink/10 bg-admin-cream p-3 text-left text-sm text-admin-ink" type="button" onclick={() => props.onDownloadProducedFile(file.id)}>
            <span class="min-w-0 truncate font-bold">{file.name}</span>
            <span class="inline-flex shrink-0 items-center gap-1 text-xs text-admin-ink/58"><Download size={13} aria-hidden="true" /> {file.description}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</section>

{#snippet Metric(label: string, value: string, testId: string)}
  <span class="min-w-0 rounded-xl bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
    {label}
    <strong class="mt-1 block truncate font-mono text-admin-ink normal-case" data-testid={testId}>{value}</strong>
  </span>
{/snippet}

{#snippet Timeline(title: string, rows: WorkflowOperationsViewModel['recentLogs'], empty: string)}
  <div class="grid min-w-0 gap-2 rounded-[16px] bg-admin-cream/55 p-3">
    <h3 class="m-0 text-sm font-extrabold text-admin-ink">{title}</h3>
    {#if rows.length === 0}
      <p class="admin-empty">{empty}</p>
    {:else}
      {#each rows as row}
        <p class="m-0 min-w-0 rounded-xl bg-admin-night/5 p-2 text-xs text-admin-ink/70">
          <strong class="block truncate text-admin-ink">{row.label}</strong>
          <span class="[overflow-wrap:anywhere]">{row.message}</span>
        </p>
      {/each}
    {/if}
  </div>
{/snippet}
