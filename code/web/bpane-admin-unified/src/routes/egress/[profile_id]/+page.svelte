<script lang="ts">
  import EgressProfileDetailRoute from '$lib/components/EgressProfileDetailRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="egress" title="BrowserPane Egress Profile Details">
  {#snippet children(authContext)}
    <EgressProfileDetailRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
