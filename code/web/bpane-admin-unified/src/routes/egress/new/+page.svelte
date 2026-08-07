<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import EgressProfileCreateRoute from '$lib/components/EgressProfileCreateRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<EgressProfileCreateRoute {authContext} />
