<script lang="ts">
  import { ArrowLeft, AlertTriangle } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import ProjectInspector from '$lib/components/ProjectInspector.svelte';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import type {
    ProjectActionState,
    ProjectDetailLoadState,
    ProjectPolicyOptionsLoadState,
  } from '$lib/projects/project-detail-state';
  import type { ProjectResource, ProjectUsageResource, UpsertProjectRequest } from '$lib/projects/project-types';

  type ProjectDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: ProjectDetailRouteProps = $props();
  let projectState = $state<ProjectDetailLoadState>({ status: 'idle' });
  let projectActionState = $state<ProjectActionState>({ status: 'idle' });
  let policyOptionsState = $state<ProjectPolicyOptionsLoadState>({ status: 'idle' });

  onMount(() => {
    const projectId = currentProjectId();
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
  });

  function client(): ProjectCatalogClient {
    return new ProjectCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadProject(projectId: string): Promise<void> {
    projectState = { status: 'loading', projectId };
    projectActionState = { status: 'idle' };
    try {
      const project = await client().getProject(projectId);
      projectState = { status: 'ready', project };
    } catch (error) {
      projectState = {
        status: 'error',
        projectId,
        message: error instanceof Error ? error.message : 'Unexpected project detail error.',
      };
    }
  }

  async function loadPolicyOptions(): Promise<void> {
    policyOptionsState = { status: 'loading' };
    try {
      const options = await client().listProjectPolicyOptions();
      policyOptionsState = { status: 'ready', options };
    } catch (error) {
      policyOptionsState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project resource selector load failed.',
      };
    }
  }

  async function refreshProject(): Promise<void> {
    const projectId = activeProjectId();
    if (!projectId) {
      return;
    }
    projectActionState = { status: 'running', label: 'Refreshing project...' };
    try {
      const project = await client().getProject(projectId);
      projectState = { status: 'ready', project };
      projectActionState = { status: 'success', message: 'Project refreshed.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project refresh failed.',
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
      const usage = await client().getProjectUsage(project.id);
      projectState = { status: 'ready', project: replaceUsage(project, usage) };
      projectActionState = { status: 'success', message: 'Project usage refreshed.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project usage refresh failed.',
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
      const project = await client().updateProject(projectId, request);
      projectState = { status: 'ready', project };
      projectActionState = { status: 'success', message: 'Project saved.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project save failed.',
      };
    }
  }

  function currentProjectId(): string | null {
    const match = window.location.pathname.match(/\/projects\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeProjectId(): string | null {
    if (projectState.status === 'ready') {
      return projectState.project.id;
    }
    if (projectState.status === 'loading' || projectState.status === 'error') {
      return projectState.projectId;
    }
    return null;
  }

  function replaceUsage(project: ProjectResource, usage: ProjectUsageResource): ProjectResource {
    return {
      ...project,
      usage,
    };
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
    <section
      class="flex items-start gap-3 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-900"
      role="alert"
      data-testid="project-detail-error"
    >
      <AlertTriangle class="mt-0.5 shrink-0" size={17} strokeWidth={1.9} />
      <div class="min-w-0">
        <p class="m-0 font-semibold">Project detail unavailable</p>
        <p class="m-0 mt-1 break-words text-red-800">{projectState.message}</p>
      </div>
    </section>
  {:else}
    <ProjectInspector
      state={projectState}
      actionState={projectActionState}
      {policyOptionsState}
      onRefreshProject={refreshProject}
      onRefreshUsage={refreshUsage}
      onSaveProject={saveProject}
    />
  {/if}
</div>
