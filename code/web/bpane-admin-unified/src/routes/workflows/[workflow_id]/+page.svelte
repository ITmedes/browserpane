<script lang="ts">
  import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider';
  import WorkflowDefinitionDetailRoute from '$lib/components/WorkflowDefinitionDetailRoute.svelte';

  const authContext = useUnifiedAdminContext();
</script>

<WorkflowDefinitionDetailRoute {authContext} />
