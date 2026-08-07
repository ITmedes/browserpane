<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import FileWorkspaceDetailRoute from '$lib/components/FileWorkspaceDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<FileWorkspaceDetailRoute {authContext} />
