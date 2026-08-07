<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import FileWorkspaceOverviewRoute from '$lib/components/FileWorkspaceOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<FileWorkspaceOverviewRoute {authContext} />
