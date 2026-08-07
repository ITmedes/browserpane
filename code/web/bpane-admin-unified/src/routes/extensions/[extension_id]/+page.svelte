<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import ExtensionDetailRoute from '$lib/components/ExtensionDetailRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>
<ExtensionDetailRoute {authContext} />
