<script lang="ts">
  import { ArrowLeft, ExternalLink, RefreshCw } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import {
    buildWorkflowRunDetailModel,
    formatWorkflowRunBytes,
    formatWorkflowRunJson,
    workflowRunDefinitionHref,
    workflowRunSessionHref,
    workflowRunSessionPreviewHref,
    type WorkflowRunDetailEvidenceState,
  } from '$lib/workflow-runs/workflow-run-detail-view-model';
  import type {
    WorkflowRunProducedFileResource,
    WorkflowRunResource,
  } from '$lib/workflow-runs/workflow-run-types';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowRunAdmissionEvidence from './WorkflowRunAdmissionEvidence.svelte';
  import WorkflowRunControls from './WorkflowRunControls.svelte';
  import WorkflowRunEvidence from './WorkflowRunEvidence.svelte';

  type WorkflowRunInspectorProps = {
    readonly run: WorkflowRunResource;
    readonly evidence: WorkflowRunDetailEvidenceState;
    readonly actionState?: AdminActionState;
    readonly downloadState?: AdminActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onCancel?: () => void | Promise<void>;
    readonly onResume?: () => void | Promise<void>;
    readonly onSubmitInput?: (input: unknown) => void | Promise<void>;
    readonly onReject?: (reason: string) => void | Promise<void>;
    readonly onDownloadProducedFile?: (
      file: WorkflowRunProducedFileResource,
    ) => void | Promise<void>;
  };

  let {
    run,
    evidence,
    actionState = { status: 'idle' },
    downloadState = { status: 'idle' },
    onRefresh,
    onCancel,
    onResume,
    onSubmitInput,
    onReject,
    onDownloadProducedFile,
  }: WorkflowRunInspectorProps = $props();

  const model = $derived(buildWorkflowRunDetailModel(run));
  const actionInFlight = $derived(actionState.status === 'running');
</script>

<article class="grid min-w-0 gap-5" data-testid="workflow-run-detail-inspector">
  <header class="flex flex-col gap-4 border-b border-admin-border pb-4 xl:flex-row xl:items-end xl:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/workflow-runs"
        data-testid="workflow-run-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} aria-hidden="true" />
        <span>Workflow runs</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate / Workflow run</p>
      <div class="mt-1 flex min-w-0 flex-wrap items-center gap-3">
        <h1 class="m-0 min-w-0 break-all font-mono text-xl font-semibold text-admin-ink" data-testid="workflow-run-detail-title">{run.id}</h1>
        <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.stateTone)}`}>
          {run.state}
        </span>
      </div>
    </div>

    <div class="flex flex-wrap gap-2">
      <a
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
        href={workflowRunDefinitionHref(run.workflow_definition_id)}
        data-testid="workflow-run-detail-workflow-link"
      >
        <span>Workflow</span>
      </a>
      {#if model.projectHref}
        <a
          class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
          href={model.projectHref}
          data-testid="workflow-run-detail-project-link"
        >
          <span>Project</span>
        </a>
      {/if}
      <a
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
        href={workflowRunSessionHref(run.session_id)}
        data-testid="workflow-run-detail-session-link"
      >
        <span>Session</span>
      </a>
      <a
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
        href={workflowRunSessionPreviewHref(run.session_id)}
        target="_blank"
        rel="noopener noreferrer"
        data-testid="workflow-run-detail-preview-link"
      >
        <ExternalLink size={15} strokeWidth={1.9} aria-hidden="true" />
        <span>Open browser</span>
      </a>
      <button
        class="inline-flex h-10 items-center gap-2 rounded-md bg-admin-accent px-3 text-sm font-medium text-white hover:brightness-95 disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        disabled={actionInFlight}
        onclick={() => void onRefresh?.()}
        data-testid="workflow-run-detail-refresh"
      >
        <RefreshCw size={16} strokeWidth={1.9} aria-hidden="true" />
        <span>Refresh</span>
      </button>
    </div>
  </header>

  <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3" aria-label="Workflow run facts">
    {#each model.facts as fact}
      <div class="min-w-0 border-l-2 border-admin-border px-3 py-1">
        <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{fact.label}</p>
        <p class="m-0 mt-1 break-words text-sm font-semibold text-admin-ink" data-testid={fact.testId}>{fact.value}</p>
      </div>
    {/each}
  </section>

  <WorkflowRunAdmissionEvidence evidence={model.admissionEvidence} />

  {#if run.error}
    <AdminMessage
      tone="error"
      title="Workflow run failed"
      message={run.error}
      testId="workflow-run-detail-terminal-error"
    />
  {/if}

  <section class="grid gap-4 border-t border-admin-border pt-4 lg:grid-cols-2" data-testid="workflow-run-detail-metadata">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Execution identity</p>
      <dl class="mt-3 grid min-w-0 grid-cols-[minmax(110px,auto)_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
        {@render Detail('Workflow', run.workflow_definition_id)}
        {@render Detail('Version resource', run.workflow_definition_version_id)}
        {@render Detail('Session', run.session_id, 'workflow-run-detail-session-id')}
        {@render Detail('Automation task', run.automation_task_id)}
        {@render Detail('Project', model.projectLabel)}
        {@render Detail('Source', model.sourceLabel)}
        {@render Detail('Client request', run.client_request_id ?? 'Not provided')}
      </dl>
    </div>
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Lifecycle</p>
      <dl class="mt-3 grid min-w-0 grid-cols-[minmax(110px,auto)_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
        {@render Detail('Admission', model.admissionLabel)}
        {@render Detail('Started', run.started_at ? formatDateTime(run.started_at) : 'Not started')}
        {@render Detail('Completed', run.completed_at ? formatDateTime(run.completed_at) : 'Not completed')}
        {@render Detail('Hold until', run.runtime?.hold_until ? formatDateTime(run.runtime.hold_until) : 'No hold')}
        {@render Detail('Runtime release', run.runtime?.release_reason ?? 'Not released')}
        {@render Detail('Logs expire', run.retention.logs_expire_at ? formatDateTime(run.retention.logs_expire_at) : 'No expiry')}
        {@render Detail('Output expires', run.retention.output_expire_at ? formatDateTime(run.retention.output_expire_at) : 'No expiry')}
      </dl>
    </div>
  </section>

  <WorkflowRunControls
    availability={model.controls}
    pendingRequest={run.intervention.pending_request}
    {actionState}
    {onCancel}
    {onResume}
    {onSubmitInput}
    {onReject}
  />

  <section class="grid gap-5 xl:grid-cols-2" data-testid="workflow-run-detail-data">
    {@render JsonSection('Input', run.input, 'workflow-run-detail-input')}
    {@render JsonSection('Output', run.output, 'workflow-run-detail-output')}
  </section>

  <section class="grid gap-5 border-t border-admin-border pt-4 xl:grid-cols-2" data-testid="workflow-run-detail-resources">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Source snapshot</p>
      {#if run.source_snapshot}
        <dl class="mt-3 grid min-w-0 grid-cols-[minmax(100px,auto)_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
          {@render Detail('Entrypoint', run.source_snapshot.entrypoint)}
          {@render Detail('Repository', run.source_snapshot.source.repository_url)}
          {@render Detail('Commit', run.source_snapshot.source.resolved_commit ?? 'Not pinned')}
          {@render Detail('Root path', run.source_snapshot.source.root_path ?? '.')}
          {@render Detail('Archive', run.source_snapshot.file_name)}
        </dl>
      {:else}
        <p class="m-0 mt-3 text-sm text-admin-muted">No source snapshot is attached.</p>
      {/if}
    </div>
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Intervention history</p>
      {#if run.intervention.last_resolution}
        <dl class="mt-3 grid min-w-0 grid-cols-[minmax(100px,auto)_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
          {@render Detail('Action', run.intervention.last_resolution.action)}
          {@render Detail('Actor', run.intervention.last_resolution.actor_display_name ?? run.intervention.last_resolution.actor_subject)}
          {@render Detail('Reason', run.intervention.last_resolution.reason ?? 'No reason')}
          {@render Detail('Resolved', formatDateTime(run.intervention.last_resolution.resolved_at))}
        </dl>
      {:else}
        <p class="m-0 mt-3 text-sm text-admin-muted">No intervention decision is retained.</p>
      {/if}
    </div>
    {@render ResourceList('Workspace inputs', run.workspace_inputs.map((input) => `${input.mount_path} / ${input.file_name} / ${formatWorkflowRunBytes(input.byte_count)}`), 'workflow-run-detail-workspace-inputs')}
    {@render ResourceList('Extensions', run.extensions.map((extension) => `${extension.name} ${extension.version}`), 'workflow-run-detail-extensions')}
    {@render ResourceList('Credential bindings', run.credential_bindings.map((binding) => `${binding.name} / ${binding.provider} / ${binding.injection_mode}`), 'workflow-run-detail-credential-bindings')}
    {@render ResourceList('Recordings', run.recordings.map((recording) => `${recording.id} / ${recording.state} / ${recording.bytes === null || recording.bytes === undefined ? 'size pending' : formatWorkflowRunBytes(recording.bytes)}`), 'workflow-run-detail-recordings')}
    {@render ResourceList('Artifact references', run.artifact_refs, 'workflow-run-detail-artifact-refs')}
    {@render ResourceList('Labels', Object.entries(run.labels).map(([key, value]) => `${key}=${value}`), 'workflow-run-detail-labels')}
  </section>

  <WorkflowRunEvidence {evidence} {downloadState} {onDownloadProducedFile} />
</article>

{#snippet Detail(label: string, value: string, testId?: string)}
  <dt class="text-admin-muted">{label}</dt>
  <dd class="m-0 min-w-0 break-all font-mono text-admin-ink" data-testid={testId}>{value}</dd>
{/snippet}

{#snippet JsonSection(title: string, value: unknown, testId: string)}
  <section class="min-w-0 border-t border-admin-border pt-4">
    <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{title}</p>
    <pre class="m-0 mt-3 max-h-80 overflow-auto rounded-md border border-admin-border bg-admin-soft p-3 text-xs leading-5 text-admin-ink" data-testid={testId}>{formatWorkflowRunJson(value)}</pre>
  </section>
{/snippet}

{#snippet ResourceList(title: string, items: readonly string[], testId: string)}
  <section class="min-w-0" data-testid={testId}>
    <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{title}</p>
    {#if items.length === 0}
      <p class="m-0 mt-3 text-sm text-admin-muted">None</p>
    {:else}
      <ul class="m-0 mt-3 grid list-none gap-2 p-0">
        {#each items as item}
          <li class="min-w-0 break-all border-l-2 border-admin-border pl-3 font-mono text-xs leading-5 text-admin-ink">{item}</li>
        {/each}
      </ul>
    {/if}
  </section>
{/snippet}
