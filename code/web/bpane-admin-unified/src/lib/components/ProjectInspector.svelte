<script lang="ts">
  import { Copy, RefreshCw } from '@lucide/svelte';
  import type {
    ProjectActionState,
    ProjectDetailLoadState,
    ProjectPolicyOptionsLoadState,
    ProjectRelatedSessionsLoadState,
    ProjectRelatedWorkflowRunsLoadState,
  } from '$lib/projects/project-detail-state';
  import { buildProjectInspectorModel } from '$lib/projects/project-inspector-view-model';
  import type { UpsertProjectRequest } from '$lib/projects/project-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import ActionFeedback from './ActionFeedback.svelte';
  import ProjectEditForm from './ProjectEditForm.svelte';
  import ProjectGovernanceEvidence from './ProjectGovernanceEvidence.svelte';

  type ProjectInspectorProps = {
    readonly state: ProjectDetailLoadState;
    readonly actionState?: ProjectActionState;
    readonly policyOptionsState?: ProjectPolicyOptionsLoadState;
    readonly relatedSessionsState?: ProjectRelatedSessionsLoadState;
    readonly relatedWorkflowRunsState?: ProjectRelatedWorkflowRunsLoadState;
    readonly onRefreshProject?: () => void | Promise<void>;
    readonly onRefreshUsage?: () => void | Promise<void>;
    readonly onRefreshRelatedWork?: () => void | Promise<void>;
    readonly onSaveProject?: (request: UpsertProjectRequest) => void | Promise<void>;
  };

  let {
    state,
    actionState = { status: 'idle' },
    policyOptionsState = { status: 'idle' },
    relatedSessionsState = { status: 'idle' },
    relatedWorkflowRunsState = { status: 'idle' },
    onRefreshProject,
    onRefreshUsage,
    onRefreshRelatedWork,
    onSaveProject,
  }: ProjectInspectorProps = $props();

  const model = $derived(state.status === 'ready' ? buildProjectInspectorModel(state.project) : null);
  const busy = $derived(actionState.status === 'running');

  function refreshProject(): void {
    void onRefreshProject?.();
  }

  function refreshUsage(): void {
    void onRefreshUsage?.();
  }

  function saveProject(request: UpsertProjectRequest): void {
    void onSaveProject?.(request);
  }

  function refreshRelatedWork(): void {
    void onRefreshRelatedWork?.();
  }

  async function copyProjectId(projectId: string): Promise<void> {
    await navigator.clipboard?.writeText(projectId);
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="project-inspector">
  {#if state.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-center text-sm text-admin-muted" data-testid="project-inspector-idle">
      Select a project to inspect and edit.
    </div>
  {:else if state.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="project-inspector-loading">
      Loading project...
    </div>
  {:else if state.status === 'error'}
    <div class="p-4">
      <AdminMessage
        tone="error"
        title="Project unavailable"
        message={state.message}
        testId="project-inspector-error"
      />
    </div>
  {:else if model}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 truncate text-xl font-semibold text-admin-ink">{model.name}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.stateTone)}`}>
              {model.state}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{model.description}</p>
          <p class="m-0 mt-2 truncate font-mono text-xs text-admin-muted">{model.id}</p>
        </div>

        <div class="flex shrink-0 flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            type="button"
            onclick={() => void copyProjectId(model.id)}
            data-testid="project-copy-id"
          >
            <Copy size={15} strokeWidth={1.8} />
            <span>Copy ID</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={refreshProject}
            disabled={busy}
            data-testid="project-refresh-detail"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Refresh</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={refreshUsage}
            disabled={busy}
            data-testid="project-refresh-usage"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Usage</span>
          </button>
        </div>
      </div>
    </div>

    <div class="border-b border-admin-border p-4">
      <ActionFeedback
        state={actionState}
        successTitle="Project action completed"
        errorTitle="Project action failed"
        successTestId="project-action-success"
        errorTestId="project-action-error"
        runningTestId="project-action-running"
      />
    </div>

    <div class="grid gap-4 p-4">
      <ProjectGovernanceEvidence
        project={state.project}
        {policyOptionsState}
        {relatedSessionsState}
        {relatedWorkflowRunsState}
        onRefreshRelatedWork={refreshRelatedWork}
      />
      {#key `${state.project.id}:${state.project.updated_at}`}
        <ProjectEditForm
          project={state.project}
          disabled={busy}
          {policyOptionsState}
          onSave={saveProject}
        />
      {/key}
    </div>
  {/if}
</aside>
