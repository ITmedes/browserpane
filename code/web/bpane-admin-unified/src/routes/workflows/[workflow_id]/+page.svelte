<script lang="ts">
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
  import WorkflowDefinitionDetailRoute from '$lib/components/WorkflowDefinitionDetailRoute.svelte';
</script>

<UnifiedAdminShell activeId="workflows" title="BrowserPane Workflow Details">
  {#snippet children(authContext)}
    <WorkflowDefinitionDetailRoute {authContext} />
  {/snippet}
</UnifiedAdminShell>
