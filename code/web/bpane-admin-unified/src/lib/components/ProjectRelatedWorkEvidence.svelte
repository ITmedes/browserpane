<script lang="ts">
  import { RefreshCw } from '@lucide/svelte';
  import type {
    ProjectRelatedSessionsLoadState,
    ProjectRelatedWorkflowRunsLoadState,
  } from '$lib/projects/project-detail-state';
  import { ProjectRelatedWorkPresenter } from '$lib/projects/project-related-work-presenter';
  import ProjectRelatedWorkList from './ProjectRelatedWorkList.svelte';

  type ProjectRelatedWorkEvidenceProps = {
    readonly projectId: string;
    readonly sessionsState?: ProjectRelatedSessionsLoadState;
    readonly workflowRunsState?: ProjectRelatedWorkflowRunsLoadState;
    readonly onRefresh?: () => void | Promise<void>;
  };

  let {
    projectId,
    sessionsState = { status: 'idle' },
    workflowRunsState = { status: 'idle' },
    onRefresh,
  }: ProjectRelatedWorkEvidenceProps = $props();
  const presenter = new ProjectRelatedWorkPresenter();
  const model = $derived(presenter.build(
    projectId,
    sessionsState.status === 'ready' ? sessionsState.sessions : [],
    workflowRunsState.status === 'ready' ? workflowRunsState.runs : [],
  ));
  const loading = $derived(
    sessionsState.status === 'loading' || workflowRunsState.status === 'loading',
  );

  function refresh(): void {
    void onRefresh?.();
  }
</script>

<section class="px-4 py-5 sm:px-5" data-testid="project-related-work-evidence">
  <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
    <div>
      <h4 class="m-0 text-sm font-semibold text-admin-ink">Related work</h4>
      <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
        Project-scoped sessions and workflow runs with authoritative admission and queue evidence.
      </p>
    </div>
    <button
      class="inline-flex h-9 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={refresh}
      disabled={loading}
      data-testid="project-related-work-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} />
      <span>Refresh work</span>
    </button>
  </div>

  <div class="mt-4 grid gap-6 lg:grid-cols-2">
    <ProjectRelatedWorkList
      title="Sessions"
      emptyMessage="No sessions are associated with this project."
      items={model.sessions}
      status={sessionsState.status}
      errorMessage={sessionsState.status === 'error' ? sessionsState.message : null}
      testId="project-related-sessions"
    />
    <ProjectRelatedWorkList
      title="Workflow runs"
      emptyMessage="No workflow runs are associated with this project."
      items={model.workflowRuns}
      status={workflowRunsState.status}
      errorMessage={workflowRunsState.status === 'error' ? workflowRunsState.message : null}
      testId="project-related-workflow-runs"
    />
  </div>
</section>
