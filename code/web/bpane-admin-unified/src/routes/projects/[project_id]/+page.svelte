<script lang="ts">
  import ProjectDetailRoute from '$lib/components/ProjectDetailRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="projects" title="BrowserPane Project Details">
  {#snippet children(authContext)}
    <ProjectDetailRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
