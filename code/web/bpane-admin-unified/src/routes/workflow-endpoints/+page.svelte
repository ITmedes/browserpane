<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowEndpointOverviewRoute from '$lib/components/WorkflowEndpointOverviewRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<WorkflowEndpointOverviewRoute {authContext} />
