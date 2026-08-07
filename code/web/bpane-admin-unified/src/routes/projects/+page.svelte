<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import ProjectOverviewRoute from '$lib/components/ProjectOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<ProjectOverviewRoute {authContext} />
