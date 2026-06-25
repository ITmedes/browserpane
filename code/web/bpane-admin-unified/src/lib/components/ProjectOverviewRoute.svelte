<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import ProjectOverview from '$lib/components/ProjectOverview.svelte';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import type { ProjectActionState, ProjectDetailLoadState } from '$lib/projects/project-detail-state';
  import type { ProjectOverviewLoadState } from '$lib/projects/project-overview-view-model';
  import type { ProjectResource, ProjectUsageResource, UpsertProjectRequest } from '$lib/projects/project-types';

  type ProjectOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: ProjectOverviewRouteProps = $props();
  let projectState = $state<ProjectOverviewLoadState>({ status: 'loading' });
  let selectedProjectState = $state<ProjectDetailLoadState>({ status: 'idle' });
  let projectActionState = $state<ProjectActionState>({ status: 'idle' });

  onMount(() => {
    void loadProjects();
  });

  function client(): ProjectCatalogClient {
    return new ProjectCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadProjects(): Promise<void> {
    projectState = { status: 'loading' };
    try {
      const response = await client().listProjects();
      projectState = {
        status: 'ready',
        projects: response.projects,
      };
      const selectedProjectId = currentProjectId();
      if (selectedProjectId) {
        await selectProject(selectedProjectId, { updateUrl: false });
      }
    } catch (error) {
      projectState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected project catalog error.',
      };
    }
  }

  async function selectProject(projectId: string, options: { readonly updateUrl?: boolean } = {}): Promise<void> {
    if (options.updateUrl ?? true) {
      updateSelectedProjectUrl(projectId);
    }
    selectedProjectState = { status: 'loading', projectId };
    projectActionState = { status: 'idle' };
    try {
      const project = await client().getProject(projectId);
      selectedProjectState = { status: 'ready', project };
      replaceProjectInList(project);
    } catch (error) {
      selectedProjectState = {
        status: 'error',
        projectId,
        message: error instanceof Error ? error.message : 'Unexpected project detail error.',
      };
    }
  }

  async function refreshSelectedProject(): Promise<void> {
    const projectId = selectedProjectId();
    if (!projectId) {
      return;
    }
    projectActionState = { status: 'running', label: 'Refreshing project...' };
    try {
      const project = await client().getProject(projectId);
      selectedProjectState = { status: 'ready', project };
      replaceProjectInList(project);
      projectActionState = { status: 'success', message: 'Project refreshed.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project refresh failed.',
      };
    }
  }

  async function refreshSelectedUsage(): Promise<void> {
    if (selectedProjectState.status !== 'ready') {
      return;
    }
    const project = selectedProjectState.project;
    projectActionState = { status: 'running', label: 'Refreshing usage...' };
    try {
      const usage = await client().getProjectUsage(project.id);
      const updatedProject = replaceProjectUsage(project, usage);
      selectedProjectState = { status: 'ready', project: updatedProject };
      replaceProjectInList(updatedProject);
      projectActionState = { status: 'success', message: 'Project usage refreshed.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project usage refresh failed.',
      };
    }
  }

  async function saveProject(request: UpsertProjectRequest): Promise<void> {
    if (selectedProjectState.status !== 'ready') {
      return;
    }
    const projectId = selectedProjectState.project.id;
    projectActionState = { status: 'running', label: 'Saving project...' };
    try {
      const project = await client().updateProject(projectId, request);
      selectedProjectState = { status: 'ready', project };
      replaceProjectInList(project);
      projectActionState = { status: 'success', message: 'Project saved.' };
    } catch (error) {
      projectActionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Project save failed.',
      };
    }
  }

  function currentProjectId(): string | null {
    return new URL(window.location.href).searchParams.get('project');
  }

  function selectedProjectId(): string | null {
    if (selectedProjectState.status === 'ready') {
      return selectedProjectState.project.id;
    }
    if (selectedProjectState.status === 'loading' || selectedProjectState.status === 'error') {
      return selectedProjectState.projectId;
    }
    return null;
  }

  function updateSelectedProjectUrl(projectId: string): void {
    const url = new URL(window.location.href);
    url.searchParams.set('project', projectId);
    history.replaceState(history.state, '', url);
  }

  function replaceProjectInList(project: ProjectResource): void {
    if (projectState.status !== 'ready') {
      return;
    }
    projectState = {
      status: 'ready',
      projects: projectState.projects.map((existing) => existing.id === project.id ? project : existing),
    };
  }

  function replaceProjectUsage(project: ProjectResource, usage: ProjectUsageResource): ProjectResource {
    return {
      ...project,
      usage,
    };
  }
</script>

<ProjectOverview
  state={projectState}
  {selectedProjectState}
  {projectActionState}
  onRefresh={loadProjects}
  onSelectProject={selectProject}
  onRefreshSelectedProject={refreshSelectedProject}
  onRefreshSelectedUsage={refreshSelectedUsage}
  onSaveProject={saveProject}
/>
