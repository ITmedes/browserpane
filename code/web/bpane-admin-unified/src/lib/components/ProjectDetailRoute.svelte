<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import AdminMessage from '$lib/components/AdminMessage.svelte';
  import ProjectInspector from '$lib/components/ProjectInspector.svelte';
  import { ProjectDetailRouteSupport } from '$lib/projects/project-detail-route-support';
  import type {
    ProjectActionState,
    ProjectDetailLoadState,
    ProjectPolicyOptionsLoadState,
    ProjectRelatedSessionsLoadState,
    ProjectRelatedWorkflowRunsLoadState,
  } from '$lib/projects/project-detail-state';
  import type { UpsertProjectRequest } from '$lib/projects/project-types';

  type ProjectDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: ProjectDetailRouteProps = $props();
  // svelte-ignore state_referenced_locally
  const support = new ProjectDetailRouteSupport(authContext);
  let projectState = $state<ProjectDetailLoadState>({ status: 'idle' });
  let projectActionState = $state<ProjectActionState>({ status: 'idle' });
  let policyOptionsState = $state<ProjectPolicyOptionsLoadState>({ status: 'idle' });
  let relatedSessionsState = $state<ProjectRelatedSessionsLoadState>({ status: 'idle' });
  let relatedWorkflowRunsState = $state<ProjectRelatedWorkflowRunsLoadState>({ status: 'idle' });

  onMount(() => {
    const projectId = support.currentProjectId(window.location.pathname);
    if (!projectId) {
      projectState = {
        status: 'error',
        projectId: 'unknown',
        message: 'Project id is missing from the current route.',
      };
      return;
    }
    void loadProject(projectId);
    void loadPolicyOptions();
    void loadRelatedSessions();
    void loadRelatedWorkflowRuns();
  });

  async function loadProject(projectId: string): Promise<void> {
    projectState = { status: 'loading', projectId };
    projectActionState = { status: 'idle' };
    try {
      const project = await support.projectClient().getProject(projectId);
      projectState = { status: 'ready', project };
    } catch (error) {
      projectState = {
        status: 'error',
        projectId,
        message: adminErrorMessage(error, 'Unexpected project detail error.'),
      };
    }
  }

  async function loadPolicyOptions(): Promise<void> {
    policyOptionsState = { status: 'loading' };
    try {
      const options = await support.projectClient().listProjectPolicyOptions();
      policyOptionsState = { status: 'ready', options };
    } catch (error) {
      policyOptionsState = {
        status: 'error',
        message: adminErrorMessage(error, 'Project resource selector load failed.'),
      };
    }
  }

  async function loadRelatedSessions(): Promise<void> {
    relatedSessionsState = { status: 'loading' };
    try {
      const response = await support.sessionClient().listSessions();
      relatedSessionsState = { status: 'ready', sessions: response.sessions };
    } catch (error) {
      relatedSessionsState = {
        status: 'error',
        message: adminErrorMessage(error, 'Related session load failed.'),
      };
    }
  }

  async function loadRelatedWorkflowRuns(): Promise<void> {
    relatedWorkflowRunsState = { status: 'loading' };
    try {
      const response = await support.workflowRunClient().listRuns();
      relatedWorkflowRunsState = { status: 'ready', runs: response.runs };
    } catch (error) {
      relatedWorkflowRunsState = {
        status: 'error',
        message: adminErrorMessage(error, 'Related workflow run load failed.'),
      };
    }
  }

  async function refreshRelatedWork(): Promise<void> {
    await Promise.all([loadRelatedSessions(), loadRelatedWorkflowRuns()]);
  }

  async function refreshProject(): Promise<void> {
    const projectId = support.activeProjectId(projectState);
    if (!projectId) {
      return;
    }
    projectActionState = { status: 'running', label: 'Refreshing project...' };
    try {
      const project = await support.projectClient().getProject(projectId);
      projectState = { status: 'ready', project };
      projectActionState = { status: 'success', message: 'Project refreshed.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Project refresh failed.'),
      };
    }
  }

  async function refreshUsage(): Promise<void> {
    if (projectState.status !== 'ready') {
      return;
    }
    const project = projectState.project;
    projectActionState = { status: 'running', label: 'Refreshing usage...' };
    try {
      const usage = await support.projectClient().getProjectUsage(project.id);
      projectState = { status: 'ready', project: support.replaceUsage(project, usage) };
      projectActionState = { status: 'success', message: 'Project usage refreshed.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Project usage refresh failed.'),
      };
    }
  }

  async function saveProject(request: UpsertProjectRequest): Promise<void> {
    if (projectState.status !== 'ready') {
      return;
    }
    const projectId = projectState.project.id;
    projectActionState = { status: 'running', label: 'Saving project...' };
    try {
      const project = await support.projectClient().updateProject(projectId, request);
      projectState = { status: 'ready', project };
      projectActionState = { status: 'success', message: 'Project saved.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Project save failed.'),
      };
    }
  }

</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="project-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/projects"
        data-testid="project-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Projects</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Project details</h1>
    </div>
  </header>

  {#if projectState.status === 'error'}
    <div
      data-testid="project-detail-error"
    >
      <AdminMessage
        tone="error"
        title="Project detail unavailable"
        message={projectState.message}
      />
    </div>
  {:else}
    <ProjectInspector
      state={projectState}
      actionState={projectActionState}
      {policyOptionsState}
      {relatedSessionsState}
      {relatedWorkflowRunsState}
      onRefreshProject={refreshProject}
      onRefreshUsage={refreshUsage}
      onRefreshRelatedWork={refreshRelatedWork}
      onSaveProject={saveProject}
    />
  {/if}
</div>
