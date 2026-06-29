<script lang="ts">
  import BrowserContextDetailRoute from '$lib/components/BrowserContextDetailRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="contexts" title="BrowserPane Browser Context Details">
  {#snippet children(authContext)}
    <BrowserContextDetailRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
