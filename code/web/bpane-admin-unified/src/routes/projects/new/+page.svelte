<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import ProjectCreateRoute from '$lib/components/ProjectCreateRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<ProjectCreateRoute {authContext} />
