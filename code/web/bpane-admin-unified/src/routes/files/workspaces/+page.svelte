<script lang="ts">
  import FileWorkspaceOverviewRoute from '$lib/components/FileWorkspaceOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="workspaces" title="BrowserPane File Workspaces">
  {#snippet children(authContext)}
    <FileWorkspaceOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
