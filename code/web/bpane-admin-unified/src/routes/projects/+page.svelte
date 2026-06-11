<script lang="ts">
  import ProjectOverviewRoute from '$lib/components/ProjectOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="projects" title="BrowserPane Projects">
  {#snippet children(authContext)}
    <ProjectOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
