<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import EgressProfileDetailRoute from '$lib/components/EgressProfileDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<EgressProfileDetailRoute {authContext} />
