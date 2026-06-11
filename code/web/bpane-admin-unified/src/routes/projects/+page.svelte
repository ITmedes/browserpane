<script lang="ts">
  import { onMount } from 'svelte';
  import ProjectOverview from '$lib/components/ProjectOverview.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
  import {
    loadStoredAdminAccessToken,
    ProjectCatalogClient,
  } from '$lib/projects/project-client';
  import type { ProjectOverviewLoadState } from '$lib/projects/project-overview-view-model';

  let projectState = $state<ProjectOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadProjects();
  });

  async function loadProjects(): Promise<void> {
    projectState = { status: 'loading' };
    try {
      const client = new ProjectCatalogClient({
        baseUrl: window.location.origin,
        accessTokenProvider: () => loadStoredAdminAccessToken(window.sessionStorage),
      });
      const response = await client.listProjects();
      projectState = {
        status: 'ready',
        projects: response.projects,
        selectedProjectId: new URL(window.location.href).searchParams.get('project'),
      };
    } catch (error) {
      projectState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected project catalog error.',
      };
    }
  }
</script>

<UnifiedAdminShell activeId="projects" title="BrowserPane Projects">
  {#snippet children()}
    <ProjectOverview state={projectState} onRefresh={loadProjects} />
  {/snippet}
</UnifiedAdminShell>
