<script lang="ts">
  import AdminBrowserContextListRoute from '$lib/application/AdminBrowserContextListRoute.svelte';
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
</script>

<AdminRouteShell title="BrowserPane Browser Contexts">
  {#snippet children(route)}
    <AdminBrowserContextListRoute controlClient={route.controlClient} />
  {/snippet}
</AdminRouteShell>
