<script lang="ts">
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
  import WorkflowRunOverviewRoute from '$lib/components/WorkflowRunOverviewRoute.svelte';
</script>

<UnifiedAdminShell activeId="runs" title="BrowserPane Workflow Runs">
  {#snippet children(authContext)}
    <WorkflowRunOverviewRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
