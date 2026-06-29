<script lang="ts">
  import FileWorkspaceCreateRoute from '$lib/components/FileWorkspaceCreateRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="workspaces" title="BrowserPane New File Workspace">
  {#snippet children(authContext)}
    <FileWorkspaceCreateRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
