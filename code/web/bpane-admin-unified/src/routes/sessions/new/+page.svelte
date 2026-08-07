<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import SessionCreateRoute from '$lib/components/SessionCreateRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<SessionCreateRoute {authContext} />
