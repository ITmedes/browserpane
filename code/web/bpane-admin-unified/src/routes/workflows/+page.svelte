<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowOverviewRoute from '$lib/components/WorkflowOverviewRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<WorkflowOverviewRoute {authContext} />
