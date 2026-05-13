<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminFileWorkspaceListRoute from '$lib/application/AdminFileWorkspaceListRoute.svelte';
</script>

<AdminRouteShell title="BrowserPane File Workspaces">
  {#snippet children(route)}
    <AdminFileWorkspaceListRoute controlClient={route.controlClient} />
  {/snippet}
</AdminRouteShell>
