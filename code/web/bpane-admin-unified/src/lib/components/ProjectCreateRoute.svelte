<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import type {
    ProjectActionState,
    ProjectPolicyOptionsLoadState,
  } from '$lib/projects/project-detail-state';
  import type { ProjectResource, UpsertProjectRequest } from '$lib/projects/project-types';
  import AdminMessage from './AdminMessage.svelte';
  import ProjectEditForm from './ProjectEditForm.svelte';

  type ProjectCreateRouteProps = {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToProject?: (project: ProjectResource) => void;
  };

  let {
    authContext,
    navigateToProject = defaultNavigateToProject,
  }: ProjectCreateRouteProps = $props();
  let projectActionState = $state<ProjectActionState>({ status: 'idle' });
  let policyOptionsState = $state<ProjectPolicyOptionsLoadState>({ status: 'idle' });

  const busy = $derived(projectActionState.status === 'running');

  onMount(() => {
    void loadPolicyOptions();
  });

  function client(): ProjectCatalogClient {
    return new ProjectCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
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

  async function createProject(request: UpsertProjectRequest): Promise<void> {
    projectActionState = { status: 'running', label: 'Creating project...' };
    try {
      const project = await client().createProject(request);
      projectActionState = { status: 'success', message: 'Project created.' };
      navigateToProject(project);
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project creation failed.',
      };
    }
  }

  function defaultNavigateToProject(project: ProjectResource): void {
    window.location.assign(`/admin-new/projects/${encodeURIComponent(project.id)}`);
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="project-create-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/projects"
        data-testid="project-create-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Projects</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Resources</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">New project</h1>
    </div>
  </header>

  {#if projectActionState.status === 'success'}
    <AdminMessage
      tone="success"
      title="Project action completed"
      message={projectActionState.message}
      testId="project-create-success"
    />
  {:else if projectActionState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Project action failed"
      message={projectActionState.message}
      testId="project-create-error"
    />
  {:else if projectActionState.status === 'running'}
    <AdminMessage
      tone="loading"
      title={projectActionState.label}
      testId="project-create-running"
    />
  {/if}

  <ProjectEditForm
    mode="create"
    disabled={busy}
    {policyOptionsState}
    onSave={createProject}
  />
</div>
