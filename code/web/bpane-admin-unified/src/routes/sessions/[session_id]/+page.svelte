<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionDetailRoute from '$lib/components/SessionDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<SessionDetailRoute {authContext} />
