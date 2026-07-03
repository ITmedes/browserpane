<script lang="ts">
  import DashboardOverviewRoute from '$lib/components/DashboardOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="dashboard" title="BrowserPane Dashboard">
  {#snippet children(authContext)}
    <DashboardOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
