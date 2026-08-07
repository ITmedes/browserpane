<script lang="ts">
  import { page } from '$app/state';
  import { resolveAdminShellRoute } from '$lib/application/admin-shell-route';
  import UnifiedAdminContextProvider from '$lib/components/UnifiedAdminContextProvider.svelte';
  import UnifiedAdminShell from '$lib/components/UnifiedAdminShell.svelte';
  import '../app.css';

  let { children: routeContent } = $props();
  const shellRoute = $derived(resolveAdminShellRoute(page.url.pathname));
</script>

{#if shellRoute}
  <UnifiedAdminShell activeId={shellRoute.activeId} title={shellRoute.title}>
    {#snippet children(authContext)}
      <UnifiedAdminContextProvider {authContext}>
        {@render routeContent()}
      </UnifiedAdminContextProvider>
    {/snippet}
  </UnifiedAdminShell>
{:else}
  {@render routeContent()}
{/if}
