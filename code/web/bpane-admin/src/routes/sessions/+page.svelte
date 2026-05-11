<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminSessionListRoute from '$lib/application/AdminSessionListRoute.svelte';
</script>

<AdminRouteShell title="BrowserPane Sessions">
  {#snippet children(route)}
    <AdminSessionListRoute controlClient={route.controlClient} />
  {/snippet}
</AdminRouteShell>
