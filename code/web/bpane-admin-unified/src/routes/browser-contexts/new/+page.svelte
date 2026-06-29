<script lang="ts">
  import BrowserContextCreateRoute from '$lib/components/BrowserContextCreateRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="contexts" title="BrowserPane New Browser Context">
  {#snippet children(authContext)}
    <BrowserContextCreateRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
