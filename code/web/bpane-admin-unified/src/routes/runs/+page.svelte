<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowRunOverviewRoute from '$lib/components/WorkflowRunOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<WorkflowRunOverviewRoute {authContext} />
