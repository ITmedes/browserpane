<script lang="ts">
  import SessionCreateRoute from '$lib/components/SessionCreateRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="sessions" title="BrowserPane New Session">
  {#snippet children(authContext)}
    <SessionCreateRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
