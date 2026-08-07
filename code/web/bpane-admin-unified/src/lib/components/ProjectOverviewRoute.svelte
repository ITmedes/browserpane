<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import ProjectOverview from '$lib/components/ProjectOverview.svelte';
  import { ProjectCatalogClient } from '$lib/projects/project-client';
  import type { ProjectOverviewLoadState } from '$lib/projects/project-overview-view-model';

  type ProjectOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: ProjectOverviewRouteProps = $props();
  let projectState = $state<ProjectOverviewLoadState>({ status: 'loading' });

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
    } catch (error) {
      projectState = {
        status: 'error',
        message: adminErrorMessage(error, 'Unexpected project catalog error.'),
      };
    }
  }
</script>

<ProjectOverview state={projectState} onRefresh={loadProjects} />
