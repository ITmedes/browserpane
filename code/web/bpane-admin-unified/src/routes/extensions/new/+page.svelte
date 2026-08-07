<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import ExtensionCreateRoute from '$lib/components/ExtensionCreateRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>
<ExtensionCreateRoute {authContext} />
