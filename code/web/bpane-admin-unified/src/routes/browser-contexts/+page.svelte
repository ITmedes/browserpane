<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import BrowserContextOverviewRoute from '$lib/components/BrowserContextOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<BrowserContextOverviewRoute {authContext} />
