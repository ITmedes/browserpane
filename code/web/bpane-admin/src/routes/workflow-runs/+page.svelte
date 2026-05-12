<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminWorkflowRunListRoute from '$lib/application/AdminWorkflowRunListRoute.svelte';
</script>

<AdminRouteShell title="BrowserPane Workflow Runs">
  {#snippet children(route)}
    <AdminWorkflowRunListRoute workflowClient={route.workflowClient} />
  {/snippet}
</AdminRouteShell>
