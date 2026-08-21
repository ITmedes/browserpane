<script lang="ts">
  import { Plus, RefreshCw, X } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import type {
    UpsertWorkflowEndpointRequest,
    WorkflowEndpointOverviewLoadState,
    WorkflowEndpointProjectOption,
  } from '$lib/workflow-endpoints/workflow-endpoint-types';
  import {
    createWorkflowEndpointDraft,
    validateWorkflowEndpointDraft,
  } from '$lib/workflow-endpoints/workflow-endpoint-view-model';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import FieldFeedback from './FieldFeedback.svelte';

  let {
    state: loadState,
    projects,
    selectedProjectId,
    actionState = { status: 'idle' },
    onSelectProject,
    onRefresh,
    onCreate,
  }: {
    readonly state: WorkflowEndpointOverviewLoadState;
    readonly projects: readonly WorkflowEndpointProjectOption[];
    readonly selectedProjectId: string;
    readonly actionState?: AdminActionState;
    readonly onSelectProject?: (projectId: string) => void | Promise<void>;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onCreate?: (request: UpsertWorkflowEndpointRequest) => void | Promise<void>;
  } = $props();

  let showCreate = $state(false);
  let draft = $state(createWorkflowEndpointDraft());
  let fieldErrors = $state<Readonly<Record<string, readonly string[]>>>({});
  const busy = $derived(actionState.status === 'running');

  function submitCreate(event: SubmitEvent): void {
    event.preventDefault();
    const validation = validateWorkflowEndpointDraft(draft);
    fieldErrors = validation.fieldErrors;
    if (validation.request) void onCreate?.(validation.request);
  }
</script>

<div
  class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
  data-testid="workflow-endpoints-overview"
>
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-end lg:justify-between">
    <div>
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflow endpoints</h1>
      <p class="m-0 mt-2 max-w-3xl text-sm text-admin-muted">
        Publish one immutable workflow as a project-scoped polling activity for an explicitly granted machine caller.
      </p>
    </div>
    <div class="flex flex-wrap items-end gap-2">
      <label class="grid gap-1 text-xs font-semibold text-admin-muted">
        Project
        <select
          class="h-10 min-w-56 rounded-md border border-admin-border bg-admin-panel px-3 text-sm text-admin-ink"
          value={selectedProjectId}
          onchange={(event) => void onSelectProject?.(event.currentTarget.value)}
          data-testid="workflow-endpoints-project-select"
        >
          {#if projects.length === 0}<option value="">No projects available</option>{/if}
          {#each projects as project}<option value={project.id}>{project.name} · {project.state}</option>{/each}
        </select>
      </label>
      <button
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60"
        type="button"
        disabled={!selectedProjectId || busy}
        onclick={() => { showCreate = !showCreate; }}
        data-testid="workflow-endpoints-new-button"
      >{#if showCreate}<X size={16} />Close{:else}<Plus size={16} />New endpoint{/if}</button>
      <button
        class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink disabled:opacity-60"
        type="button"
        disabled={!selectedProjectId || loadState.status === 'loading'}
        onclick={() => void onRefresh?.()}
        data-testid="workflow-endpoints-refresh-button"
      ><RefreshCw size={16} />Refresh</button>
    </div>
  </header>

  <ActionFeedback
    state={actionState}
    successTitle="Workflow endpoint action completed"
    errorTitle="Workflow endpoint action failed"
    successTestId="workflow-endpoints-action-success"
    errorTestId="workflow-endpoints-action-error"
    runningTestId="workflow-endpoints-action-running"
    reserveSpace={false}
  />

  {#if showCreate}
    <form
      class="grid gap-4 rounded-md border border-admin-border bg-admin-panel p-4"
      onsubmit={submitCreate}
      data-testid="workflow-endpoint-create-form"
    >
      <div>
        <h2 class="m-0 text-base font-semibold text-admin-ink">New draft endpoint</h2>
        <p class="m-0 mt-1 text-sm text-admin-muted">Creation never activates the endpoint or grants a caller.</p>
      </div>
      <div class="grid gap-3 md:grid-cols-2">
        <label class="text-sm font-medium text-admin-ink">Endpoint key
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3 font-mono text-sm" bind:value={draft.endpointKey} data-testid="workflow-endpoint-field-key" />
          <FieldFeedback errors={fieldErrors.endpointKey} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Purpose
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3 text-sm" bind:value={draft.purpose} />
          <FieldFeedback errors={fieldErrors.purpose} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Workflow definition id
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3 font-mono text-sm" bind:value={draft.workflowDefinitionId} />
          <FieldFeedback errors={fieldErrors.workflowDefinitionId} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Immutable workflow version id
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3 font-mono text-sm" bind:value={draft.workflowDefinitionVersionId} />
          <FieldFeedback errors={fieldErrors.workflowDefinitionVersionId} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Workflow version
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3 font-mono text-sm" bind:value={draft.workflowVersion} />
          <FieldFeedback errors={fieldErrors.workflowVersion} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Labels (key=value)
          <textarea class="mt-1 min-h-24 w-full rounded-md border border-admin-border p-3 font-mono text-xs" bind:value={draft.labelsText}></textarea>
          <FieldFeedback errors={fieldErrors.labelsText} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Input schema · Draft 2020-12
          <textarea class="mt-1 min-h-48 w-full rounded-md border border-admin-border p-3 font-mono text-xs" bind:value={draft.inputSchemaText}></textarea>
          <FieldFeedback errors={fieldErrors.inputSchemaText} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Output schema · Draft 2020-12
          <textarea class="mt-1 min-h-48 w-full rounded-md border border-admin-border p-3 font-mono text-xs" bind:value={draft.outputSchemaText}></textarea>
          <FieldFeedback errors={fieldErrors.outputSchemaText} />
        </label>
      </div>
      <div class="grid gap-3 sm:grid-cols-3">
        <label class="text-sm font-medium text-admin-ink">Execution timeout (seconds)
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3" type="number" bind:value={draft.executionTimeoutSeconds} />
          <FieldFeedback errors={fieldErrors.executionTimeoutSeconds} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Inline result maximum (bytes)
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3" type="number" bind:value={draft.inlineResultMaxBytes} />
          <FieldFeedback errors={fieldErrors.inlineResultMaxBytes} />
        </label>
        <label class="text-sm font-medium text-admin-ink">Artifact retention (seconds)
          <input class="mt-1 h-10 w-full rounded-md border border-admin-border px-3" type="number" bind:value={draft.artifactRetentionSeconds} />
          <FieldFeedback errors={fieldErrors.artifactRetentionSeconds} />
        </label>
      </div>
      <div class="flex justify-end">
        <button class="h-10 rounded-md bg-admin-accent px-4 text-sm font-semibold text-white disabled:opacity-60" type="submit" disabled={busy} data-testid="workflow-endpoint-create-submit">Create draft</button>
      </div>
    </form>
  {/if}

  {#if projects.length === 0}
    <AdminMessage tone="warning" title="No projects available" message="Create an active project before publishing a workflow endpoint." testId="workflow-endpoints-no-projects" />
  {:else if loadState.status === 'loading'}
    <AdminMessage tone="loading" title="Loading workflow endpoints" message="Project endpoint metadata is being refreshed." testId="workflow-endpoints-loading" />
  {:else if loadState.status === 'error'}
    <AdminMessage tone="error" title="Workflow endpoint catalog unavailable" message={loadState.message} testId="workflow-endpoints-error" />
  {:else if loadState.workflowEndpoints.length === 0}
    <section class="flex min-h-56 items-center justify-center rounded-md border border-dashed border-admin-border bg-admin-panel text-sm text-admin-muted" data-testid="workflow-endpoints-empty">No workflow endpoints exist in this project.</section>
  {:else}
    <div class="overflow-x-auto rounded-md border border-admin-border bg-admin-panel">
      <table class="w-full border-collapse text-left text-sm">
        <thead class="bg-admin-soft text-xs uppercase text-admin-muted"><tr><th class="p-3">Endpoint</th><th class="p-3">Workflow</th><th class="p-3">State</th><th class="p-3">Limits</th><th class="p-3">Updated</th></tr></thead>
        <tbody>
          {#each loadState.workflowEndpoints as endpoint}
            <tr class="border-t border-admin-border" data-testid="workflow-endpoint-row">
              <td class="p-3"><a class="font-semibold text-admin-accent hover:underline" href={`/admin-new/workflow-endpoints/${endpoint.project_id}/${encodeURIComponent(endpoint.endpoint_key)}`}>{endpoint.endpoint_key}</a><p class="m-0 mt-1 max-w-xl text-xs text-admin-muted">{endpoint.purpose}</p></td>
              <td class="p-3"><span class="font-mono text-xs">{endpoint.workflow_version}</span></td>
              <td class="p-3 capitalize">{endpoint.state}</td>
              <td class="p-3 text-xs">{endpoint.execution_timeout_seconds}s · {endpoint.inline_result_max_bytes} bytes inline</td>
              <td class="p-3 text-xs">{new Date(endpoint.updated_at).toLocaleString()}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
