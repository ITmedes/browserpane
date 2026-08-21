<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowEndpointDetailRoute from '$lib/components/WorkflowEndpointDetailRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<WorkflowEndpointDetailRoute {authContext} />
