<script lang="ts">
  import EgressProfileOverviewRoute from '$lib/components/EgressProfileOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="egress" title="BrowserPane Egress Profiles">
  {#snippet children(authContext)}
    <EgressProfileOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
