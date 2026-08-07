<script lang="ts">
  import { onMount } from 'svelte';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { WorkflowEventCatalogClient } from '$lib/workflow-events/workflow-event-client';
  import type { WorkflowEventOverviewLoadState } from '$lib/workflow-events/workflow-event-view-model';
  import WorkflowEventSubscriptionOverview from './WorkflowEventSubscriptionOverview.svelte';
  let { authContext }: { readonly authContext: UnifiedAdminContext } = $props();
  let subscriptionState = $state<WorkflowEventOverviewLoadState>({ status: 'loading' });
  onMount(() => {
    void loadSubscriptions();
  });
  function client() {
    return new WorkflowEventCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }
  async function loadSubscriptions(): Promise<void> {
    subscriptionState = { status: 'loading' };
    try {
      subscriptionState = {
        status: 'ready',
        subscriptions: (await client().listSubscriptions()).subscriptions,
      };
    } catch (error) {
      subscriptionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Unexpected workflow event subscription error.'),
      };
    }
  }
</script>

<WorkflowEventSubscriptionOverview state={subscriptionState} onRefresh={loadSubscriptions} />
