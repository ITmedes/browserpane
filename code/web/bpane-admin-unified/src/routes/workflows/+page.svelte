<script lang="ts">
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
  import WorkflowOverviewRoute from '$lib/components/WorkflowOverviewRoute.svelte';
</script>

<UnifiedAdminShell activeId="workflows" title="BrowserPane Workflows">
  {#snippet children(authContext)}
    <WorkflowOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
