<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminSessionSurface from '$lib/application/AdminSessionSurface.svelte';
</script>

<AdminRouteShell title="BrowserPane Admin" showAdminToggle={true}>
  {#snippet children(route)}
    <AdminSessionSurface
      controlClient={route.controlClient}
      adminEventClient={route.adminEventClient}
      workflowClient={route.workflowClient}
      adminOpen={route.adminOpen}
      mcpBridge={route.authConfig?.mcpBridge ?? null}
      onAdminOpenChange={route.setAdminOpen}
    />
  {/snippet}
</AdminRouteShell>
