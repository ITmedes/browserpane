<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminFileWorkspaceDetailRoute from '$lib/application/AdminFileWorkspaceDetailRoute.svelte';

  let { data }: { data: { workspaceId: string } } = $props();
</script>

<AdminRouteShell title="BrowserPane File Workspace Detail">
  {#snippet children(route)}
    <AdminFileWorkspaceDetailRoute
      controlClient={route.controlClient}
      workspaceId={data.workspaceId}
    />
  {/snippet}
</AdminRouteShell>
