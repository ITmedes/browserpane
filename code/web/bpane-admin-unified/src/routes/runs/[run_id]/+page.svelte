<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowRunDetailRoute from '$lib/components/WorkflowRunDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<WorkflowRunDetailRoute {authContext} />
