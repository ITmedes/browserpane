<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowEventSubscriptionDetailRoute from '$lib/components/WorkflowEventSubscriptionDetailRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<WorkflowEventSubscriptionDetailRoute {authContext} />
