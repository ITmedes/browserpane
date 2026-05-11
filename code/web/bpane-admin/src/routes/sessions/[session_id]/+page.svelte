<script lang="ts">
  import AdminRouteShell from '$lib/application/AdminRouteShell.svelte';
  import AdminSessionDetailRoute from '$lib/application/AdminSessionDetailRoute.svelte';

  let { data }: { data: { sessionId: string } } = $props();
</script>

<AdminRouteShell title="BrowserPane Session Detail">
  {#snippet children(route)}
    <AdminSessionDetailRoute controlClient={route.controlClient} sessionId={data.sessionId} />
  {/snippet}
</AdminRouteShell>
