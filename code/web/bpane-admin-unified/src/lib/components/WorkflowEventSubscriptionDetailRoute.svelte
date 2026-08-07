<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { adminErrorMessage, type AdminActionState } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import { WorkflowEventCatalogClient } from '$lib/workflow-events/workflow-event-client';
  import type { WorkflowEventDetailLoadState } from '$lib/workflow-events/workflow-event-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowEventSubscriptionInspector from './WorkflowEventSubscriptionInspector.svelte';
  let {
    authContext,
    navigateToCatalog = async () => goto('/admin-new/workflow-event-subscriptions'),
  }: {
    readonly authContext: UnifiedAdminContext;
    readonly navigateToCatalog?: () => void | Promise<void>;
  } = $props();
  let subscriptionState = $state<WorkflowEventDetailLoadState>({ status: 'idle' });
  let actionState = $state<AdminActionState>({ status: 'idle' });
  onMount(() => {
    const id = currentSubscriptionId();
    if (!id) {
      subscriptionState = {
        status: 'error',
        subscriptionId: 'unknown',
        message: 'Event subscription id is missing from the current route.',
      };
      return;
    }
    void loadSubscription(id);
  });
  function client() {
    return new WorkflowEventCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }
  async function fetchDetail(id: string) {
    const eventClient = client();
    const [subscription, deliveries] = await Promise.all([
      eventClient.getSubscription(id),
      eventClient.listDeliveries(id),
    ]);
    return { subscription, deliveries: deliveries.deliveries };
  }
  async function loadSubscription(id: string): Promise<void> {
    subscriptionState = { status: 'loading', subscriptionId: id };
    try {
      subscriptionState = { status: 'ready', ...(await fetchDetail(id)) };
    } catch (error) {
      subscriptionState = {
        status: 'error',
        subscriptionId: id,
        message: adminErrorMessage(error, 'Unexpected event subscription detail error.'),
      };
    }
  }
  async function refreshSubscription(): Promise<void> {
    const id = activeSubscriptionId();
    if (!id) return;
    actionState = { status: 'running', label: 'Refreshing subscription and deliveries...' };
    try {
      subscriptionState = { status: 'ready', ...(await fetchDetail(id)) };
      actionState = { status: 'success', message: 'Subscription delivery diagnostics refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Event subscription refresh failed.'),
      };
    }
  }
  async function deleteSubscription(): Promise<void> {
    const id = activeSubscriptionId();
    if (!id) return;
    actionState = { status: 'running', label: 'Deleting event subscription...' };
    try {
      await client().deleteSubscription(id);
      actionState = { status: 'success', message: 'Event subscription deleted.' };
      await navigateToCatalog();
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Event subscription delete failed.'),
      };
    }
  }
  function currentSubscriptionId(): string | null {
    const match = window.location.pathname.match(/\/workflow-event-subscriptions\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }
  function activeSubscriptionId(): string | null {
    if (subscriptionState.status === 'ready') return subscriptionState.subscription.id;
    if (subscriptionState.status === 'loading' || subscriptionState.status === 'error')
      return subscriptionState.subscriptionId;
    return null;
  }
</script>

<div
  class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8"
  data-testid="workflow-event-subscription-detail-route"
>
  <header class="border-b border-admin-border pb-4">
    <a
      class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
      href="/admin-new/workflow-event-subscriptions"
      ><ArrowLeft size={16} strokeWidth={1.8} /><span>Workflow event subscriptions</span></a
    >
    <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Govern</p>
    <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Event subscription details</h1>
  </header>
  {#if subscriptionState.status === 'error'}<AdminMessage
      tone="error"
      title="Event subscription detail unavailable"
      message={subscriptionState.message}
      testId="workflow-event-subscription-detail-error"
    />{:else}<WorkflowEventSubscriptionInspector
      state={subscriptionState}
      {actionState}
      onRefresh={refreshSubscription}
      onDelete={deleteSubscription}
    />{/if}
</div>
