<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionLiveRoute from '$lib/components/SessionLiveRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<SessionLiveRoute {authContext} />
