<script lang="ts">
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { FileWorkspaceCatalogClient } from '$lib/file-workspaces/file-workspace-client';
  import type {
    FileWorkspaceFileCountMap,
    FileWorkspaceOverviewLoadState,
  } from '$lib/file-workspaces/file-workspace-overview-view-model';
  import type { FileWorkspaceResource } from '$lib/file-workspaces/file-workspace-types';
  import FileWorkspaceOverview from './FileWorkspaceOverview.svelte';

  type FileWorkspaceOverviewRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: FileWorkspaceOverviewRouteProps = $props();
  let workspaceState = $state<FileWorkspaceOverviewLoadState>({ status: 'loading' });

  onMount(() => {
    void loadWorkspaces();
  });

  function client(): FileWorkspaceCatalogClient {
    return new FileWorkspaceCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadWorkspaces(): Promise<void> {
    workspaceState = { status: 'loading' };
    try {
      const workspaceClient = client();
      const response = await workspaceClient.listFileWorkspaces();
      workspaceState = {
        status: 'ready',
        workspaces: response.workspaces,
        fileCounts: await loadFileCounts(workspaceClient, response.workspaces),
      };
    } catch (error) {
      workspaceState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Unexpected file workspace catalog error.',
      };
    }
  }

  async function loadFileCounts(
    workspaceClient: FileWorkspaceCatalogClient,
    workspaces: readonly FileWorkspaceResource[],
  ): Promise<FileWorkspaceFileCountMap> {
    const entries = await Promise.all(workspaces.map(async (workspace) => {
      try {
        const files = await workspaceClient.listFileWorkspaceFiles(workspace.id);
        return [workspace.id, files.files.length] as const;
      } catch {
        return [workspace.id, null] as const;
      }
    }));
    return Object.fromEntries(entries);
  }
</script>

<FileWorkspaceOverview state={workspaceState} onRefresh={loadWorkspaces} />
