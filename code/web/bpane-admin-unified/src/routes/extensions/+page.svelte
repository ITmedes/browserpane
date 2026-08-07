<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import ExtensionOverviewRoute from '$lib/components/ExtensionOverviewRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>
<ExtensionOverviewRoute {authContext} />
