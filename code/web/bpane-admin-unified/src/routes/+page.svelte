<script lang="ts">
  import DashboardOverviewRoute from '$lib/components/DashboardOverviewRoute.svelte';
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';

  const authContext = useUnifiedAdminContext();
</script>

<DashboardOverviewRoute {authContext} />
