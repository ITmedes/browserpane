<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import BrowserContextCloneRoute from '$lib/components/BrowserContextCloneRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<BrowserContextCloneRoute {authContext} />
