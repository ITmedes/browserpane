<script lang="ts">
  import SessionDetailRoute from '$lib/components/SessionDetailRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="sessions" title="BrowserPane Session Details">
  {#snippet children(authContext)}
    <SessionDetailRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
