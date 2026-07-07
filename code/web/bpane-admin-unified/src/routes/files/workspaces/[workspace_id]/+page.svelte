<script lang="ts">
  import FileWorkspaceDetailRoute from '$lib/components/FileWorkspaceDetailRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="workspaces" title="BrowserPane File Workspace Details">
  {#snippet children(authContext)}
    <FileWorkspaceDetailRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
