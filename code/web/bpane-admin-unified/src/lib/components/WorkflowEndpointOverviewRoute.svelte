<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import { WorkflowEndpointCatalogClient } from '$lib/workflow-endpoints/workflow-endpoint-client';
  import type {
    UpsertWorkflowEndpointRequest,
    WorkflowEndpointOverviewLoadState,
    WorkflowEndpointProjectOption,
  } from '$lib/workflow-endpoints/workflow-endpoint-types';
  import WorkflowEndpointOverview from './WorkflowEndpointOverview.svelte';

  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let state = $state<WorkflowEndpointOverviewLoadState>({ status: 'loading' });
  let projects = $state<readonly WorkflowEndpointProjectOption[]>([]);
  let selectedProjectId = $state('');
  let actionState = $state<AdminActionState>({ status: 'idle' });

  onMount(() => { void loadProjectsAndEndpoints(); });

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  function endpointClient(): WorkflowEndpointCatalogClient {
    return new WorkflowEndpointCatalogClient(clientOptions());
  }

  async function loadProjectsAndEndpoints(): Promise<void> {
    state = { status: 'loading' };
    try {
      const response = await new ProjectCatalogClient(clientOptions()).listProjects();
      projects = response.projects.map((project) => ({
        id: project.id,
        name: project.name,
        state: project.state,
      }));
      const requested = new URL(window.location.href).searchParams.get('project_id');
      selectedProjectId = projects.some((project) => project.id === requested)
        ? requested!
        : (projects.find((project) => project.state === 'active')?.id ?? projects[0]?.id ?? '');
      if (!selectedProjectId) {
        state = { status: 'ready', projectId: '', workflowEndpoints: [] };
        return;
      }
      await loadEndpoints(selectedProjectId);
    } catch (error) {
      state = { status: 'error', message: adminErrorMessage(error, 'Workflow endpoint catalog failed.') };
    }
  }

  async function loadEndpoints(projectId: string): Promise<void> {
    state = { status: 'loading' };
    try {
      const response = await endpointClient().listEndpoints(projectId);
      state = { status: 'ready', projectId, workflowEndpoints: response.workflow_endpoints };
    } catch (error) {
      state = { status: 'error', message: adminErrorMessage(error, 'Workflow endpoint catalog failed.') };
    }
  }

  async function selectProject(projectId: string): Promise<void> {
    selectedProjectId = projectId;
    actionState = { status: 'idle' };
    const url = new URL(window.location.href);
    if (projectId) url.searchParams.set('project_id', projectId);
    else url.searchParams.delete('project_id');
    window.history.replaceState({}, '', url);
    if (projectId) await loadEndpoints(projectId);
  }

  async function createEndpoint(request: UpsertWorkflowEndpointRequest): Promise<void> {
    if (!selectedProjectId) return;
    actionState = { status: 'running', label: 'Creating draft workflow endpoint...' };
    try {
      const created = await endpointClient().createEndpoint(selectedProjectId, request);
      await loadEndpoints(selectedProjectId);
      actionState = { status: 'success', message: `Draft endpoint ${created.endpoint_key} created. Add a caller grant before activation.` };
    } catch (error) {
      actionState = { status: 'error', message: adminErrorMessage(error, 'Workflow endpoint creation failed.') };
    }
  }
</script>

<WorkflowEndpointOverview
  {state}
  {projects}
  {selectedProjectId}
  {actionState}
  onSelectProject={selectProject}
  onRefresh={() => selectedProjectId ? loadEndpoints(selectedProjectId) : loadProjectsAndEndpoints()}
  onCreate={createEndpoint}
/>
