<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import BrowserContextImportRoute from '$lib/components/BrowserContextImportRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<BrowserContextImportRoute {authContext} />
