<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminWorkflowDefinitionDetailRoute from '$lib/application/AdminWorkflowDefinitionDetailRoute.svelte';

  let { data }: { data: { workflowId: string } } = $props();
</script>

<AdminRouteShell title="BrowserPane Workflow Template Detail">
  {#snippet children(route)}
    <AdminWorkflowDefinitionDetailRoute
      workflowClient={route.workflowClient}
      workflowId={data.workflowId}
    />
  {/snippet}
</AdminRouteShell>
