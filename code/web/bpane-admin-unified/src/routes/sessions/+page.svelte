<script lang="ts">
  import SessionOverviewRoute from '$lib/components/SessionOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="sessions" title="BrowserPane Sessions">
  {#snippet children(authContext)}
    <SessionOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
