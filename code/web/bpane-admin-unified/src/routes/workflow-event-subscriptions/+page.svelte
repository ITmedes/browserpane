<script lang="ts">import { useUnifiedAdminContext } from '$lib/auth/unified-admin-context-provider'; import WorkflowEventSubscriptionOverviewRoute from '$lib/components/WorkflowEventSubscriptionOverviewRoute.svelte'; const authContext = useUnifiedAdminContext();</script>
<WorkflowEventSubscriptionOverviewRoute {authContext} />
