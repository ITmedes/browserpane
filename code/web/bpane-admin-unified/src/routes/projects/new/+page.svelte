<script lang="ts">
  import ProjectCreateRoute from '$lib/components/ProjectCreateRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="projects" title="BrowserPane New Project">
  {#snippet children(authContext)}
    <ProjectCreateRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
