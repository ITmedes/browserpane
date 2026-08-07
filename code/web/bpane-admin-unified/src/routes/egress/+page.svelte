<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import EgressProfileOverviewRoute from '$lib/components/EgressProfileOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<EgressProfileOverviewRoute {authContext} />
