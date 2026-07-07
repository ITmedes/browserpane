<script lang="ts">
  import EgressProfileCreateRoute from '$lib/components/EgressProfileCreateRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="egress" title="BrowserPane New Egress Profile">
  {#snippet children(authContext)}
    <EgressProfileCreateRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
