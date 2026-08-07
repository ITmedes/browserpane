<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import BrowserContextCreateRoute from '$lib/components/BrowserContextCreateRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<BrowserContextCreateRoute {authContext} />
