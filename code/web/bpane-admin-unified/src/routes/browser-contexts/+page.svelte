<script lang="ts">
  import BrowserContextOverviewRoute from '$lib/components/BrowserContextOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="contexts" title="BrowserPane Browser Contexts">
  {#snippet children(authContext)}
    <BrowserContextOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
