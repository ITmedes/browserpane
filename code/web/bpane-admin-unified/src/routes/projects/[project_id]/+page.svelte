<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import ProjectDetailRoute from '$lib/components/ProjectDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<ProjectDetailRoute {authContext} />
