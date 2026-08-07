<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionOverviewRoute from '$lib/components/SessionOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<SessionOverviewRoute {authContext} />
