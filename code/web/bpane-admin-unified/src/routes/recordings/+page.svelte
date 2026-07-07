<script lang="ts">
  import RecordingOverviewRoute from '$lib/components/RecordingOverviewRoute.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
</script>

<UnifiedAdminShell activeId="recordings" title="BrowserPane Recordings">
  {#snippet children(authContext)}
    <RecordingOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
