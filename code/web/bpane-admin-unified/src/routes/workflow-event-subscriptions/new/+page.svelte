<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowEventSubscriptionCreateRoute from '$lib/components/WorkflowEventSubscriptionCreateRoute.svelte';
  const authContext = useUnifiedAdminContext();
</script>

<WorkflowEventSubscriptionCreateRoute {authContext} />
