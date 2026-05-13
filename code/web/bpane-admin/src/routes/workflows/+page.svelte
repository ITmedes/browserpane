<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminWorkflowCatalogRoute from '$lib/application/AdminWorkflowCatalogRoute.svelte';
</script>

<AdminRouteShell title="BrowserPane Workflow Catalog">
  {#snippet children(route)}
    <AdminWorkflowCatalogRoute workflowClient={route.workflowClient} />
  {/snippet}
</AdminRouteShell>
