<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminWorkflowRunDetailRoute from '$lib/application/AdminWorkflowRunDetailRoute.svelte';

  let { data }: { data: { runId: string } } = $props();
</script>

<AdminRouteShell title="BrowserPane Workflow Run Detail">
  {#snippet children(route)}
    <AdminWorkflowRunDetailRoute workflowClient={route.workflowClient} runId={data.runId} />
  {/snippet}
</AdminRouteShell>
