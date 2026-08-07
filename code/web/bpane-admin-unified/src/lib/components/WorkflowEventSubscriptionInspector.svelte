<script lang="ts">
  import { Copy, RefreshCw, Trash2 } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import type { WorkflowEventDetailLoadState } from '$lib/workflow-events/workflow-event-view-model';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowEventDeliveryDiagnostics from './WorkflowEventDeliveryDiagnostics.svelte';
  let {
    state: loadState,
    actionState = { status: 'idle' },
    onRefresh,
    onDelete,
  }: {
    readonly state: WorkflowEventDetailLoadState;
    readonly actionState?: AdminActionState;
    readonly onRefresh?: () => void | Promise<void>;
    readonly onDelete?: () => void | Promise<void>;
  } = $props();
  let confirmDelete = $state(false);
  const busy = $derived(actionState.status === 'running');
</script>

<aside
  class="min-w-0 rounded-md border border-admin-border bg-admin-panel"
  data-testid="workflow-event-subscription-inspector"
>
  {#if loadState.status === 'idle'}<div
      class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted"
    >
      Select an event subscription to inspect.
    </div>{:else if loadState.status === 'loading'}<div
      class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted"
      data-testid="workflow-event-subscription-inspector-loading"
    >
      Loading event subscription...
    </div>{:else if loadState.status === 'error'}<div class="p-4">
      <AdminMessage
        tone="error"
        title="Event subscription unavailable"
        message={loadState.message}
        testId="workflow-event-subscription-inspector-error"
      />
    </div>{:else}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <h2
              class="m-0 text-xl font-semibold text-admin-ink"
              data-testid="workflow-event-subscription-detail-name"
            >
              {loadState.subscription.name}
            </h2>
            <span
              class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(loadState.subscription.has_signing_secret ? 'success' : 'warning')}`}
              >{loadState.subscription.has_signing_secret
                ? 'Signing configured'
                : 'Signing unavailable'}</span
            >
          </div>
          <p class="m-0 mt-1 break-all text-sm text-admin-muted">
            {loadState.subscription.target_url}
          </p>
          <p class="m-0 mt-2 truncate font-mono text-xs text-admin-muted">
            {loadState.subscription.id}
          </p>
        </div>
        <div class="flex flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            type="button"
            onclick={() => void navigator.clipboard?.writeText(loadState.subscription.id)}
            data-testid="workflow-event-subscription-copy-id"
            ><Copy size={15} strokeWidth={1.8} /><span>Copy ID</span></button
          ><button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:opacity-60"
            type="button"
            onclick={() => void onRefresh?.()}
            disabled={busy}
            data-testid="workflow-event-subscription-refresh"
            ><RefreshCw size={15} strokeWidth={1.8} /><span>Refresh</span></button
          ><button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-rose-200 bg-rose-50 px-3 text-sm font-medium text-rose-800 hover:bg-rose-100 disabled:opacity-60"
            type="button"
            onclick={() => {
              confirmDelete = true;
            }}
            disabled={busy}
            data-testid="workflow-event-subscription-delete"
            ><Trash2 size={15} strokeWidth={1.8} /><span>Delete</span></button
          >
        </div>
      </div>
    </div>
    {#if confirmDelete}<div
        class="border-b border-admin-border bg-rose-50 p-4"
        data-testid="workflow-event-subscription-delete-confirm"
      >
        <AdminMessage
          tone="warning"
          title="Delete this subscription?"
          message="Future workflow events will no longer be delivered to this endpoint."
        />
        <div class="mt-3 flex justify-end gap-2">
          <button
            class="h-9 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink"
            type="button"
            onclick={() => {
              confirmDelete = false;
            }}
            data-testid="workflow-event-subscription-delete-cancel">Cancel</button
          ><button
            class="h-9 rounded-md border border-rose-700 bg-rose-700 px-3 text-sm font-semibold text-white"
            type="button"
            onclick={() => void onDelete?.()}
            data-testid="workflow-event-subscription-delete-confirm-button"
            >Delete subscription</button
          >
        </div>
      </div>{/if}
    <div class="border-b border-admin-border p-4">
      <ActionFeedback
        state={actionState}
        successTitle="Event subscription action completed"
        errorTitle="Event subscription action failed"
        successTestId="workflow-event-subscription-action-success"
        errorTestId="workflow-event-subscription-action-error"
        runningTestId="workflow-event-subscription-action-running"
      />
    </div>
    <div class="grid gap-4 p-4">
      <AdminMessage
        tone="info"
        title="Signing secret remains write-only"
        message="Only the presence of signing material is exposed after creation."
        testId="workflow-event-subscription-write-only"
      />
      <section
        class="rounded-md border border-admin-border bg-admin-soft/50 p-4"
        data-testid="workflow-event-subscription-metadata"
      >
        <h3 class="m-0 text-sm font-semibold text-admin-ink">Subscription metadata</h3>
        <dl class="mt-3 grid gap-3 md:grid-cols-2">
          <div class="rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Event filters</dt>
            <dd class="m-0 mt-1 break-words font-mono text-xs text-admin-ink">
              {loadState.subscription.event_types.join(', ')}
            </dd>
          </div>
          <div class="rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Updated</dt>
            <dd class="m-0 mt-1 text-sm text-admin-ink">
              {formatDateTime(loadState.subscription.updated_at)}
            </dd>
          </div>
        </dl>
      </section>
      <WorkflowEventDeliveryDiagnostics deliveries={loadState.deliveries} />
    </div>
  {/if}
</aside>
