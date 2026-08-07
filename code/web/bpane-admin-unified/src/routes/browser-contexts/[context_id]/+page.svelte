<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import BrowserContextDetailRoute from '$lib/components/BrowserContextDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<BrowserContextDetailRoute {authContext} />
