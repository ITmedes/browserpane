<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import FileWorkspaceCreateRoute from '$lib/components/FileWorkspaceCreateRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<FileWorkspaceCreateRoute {authContext} />
