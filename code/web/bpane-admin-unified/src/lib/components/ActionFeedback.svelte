<script lang="ts">
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import AdminMessage from './AdminMessage.svelte';

  type ActionFeedbackProps = {
    readonly state: AdminActionState;
    readonly successTitle: string;
    readonly errorTitle: string;
    readonly successTestId: string;
    readonly errorTestId: string;
    readonly runningTestId: string;
    readonly reserveSpace?: boolean;
  };

  let {
    state,
    successTitle,
    errorTitle,
    successTestId,
    errorTestId,
    runningTestId,
    reserveSpace = true,
  }: ActionFeedbackProps = $props();
</script>

<div class={reserveSpace ? 'min-h-14' : ''} data-testid="action-feedback-slot">
  {#if state.status === 'success'}
    <AdminMessage tone="success" title={successTitle} message={state.message} testId={successTestId} />
  {:else if state.status === 'error'}
    <AdminMessage tone="error" title={errorTitle} message={state.message} testId={errorTestId} />
  {:else if state.status === 'running'}
    <AdminMessage tone="loading" title={state.label} testId={runningTestId} />
  {/if}
</div>
