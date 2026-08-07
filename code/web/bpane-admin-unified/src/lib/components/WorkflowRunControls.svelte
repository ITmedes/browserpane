<script lang="ts">
  import { Ban, Play, Send, XCircle } from '@lucide/svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import type { WorkflowRunControlAvailability } from '$lib/workflow-runs/workflow-run-detail-view-model';
  import type { WorkflowRunInterventionRequestResource } from '$lib/workflow-runs/workflow-run-types';
  import ActionFeedback from './ActionFeedback.svelte';
  import AdminMessage from './AdminMessage.svelte';

  type WorkflowRunControlsProps = {
    readonly availability: WorkflowRunControlAvailability;
    readonly pendingRequest?: WorkflowRunInterventionRequestResource | null;
    readonly actionState?: AdminActionState;
    readonly onCancel?: () => void | Promise<void>;
    readonly onResume?: () => void | Promise<void>;
    readonly onSubmitInput?: (input: unknown) => void | Promise<void>;
    readonly onReject?: (reason: string) => void | Promise<void>;
  };

  let {
    availability,
    pendingRequest = null,
    actionState = { status: 'idle' },
    onCancel,
    onResume,
    onSubmitInput,
    onReject,
  }: WorkflowRunControlsProps = $props();

  let operatorInput = $state('{}');
  let rejectReason = $state('');
  let localValidationMessage = $state<string | null>(null);

  const actionInFlight = $derived(actionState.status === 'running');
  const operatorInputValid = $derived(isValidJson(operatorInput));
  const canResolve = $derived(availability.canResolveIntervention && !actionInFlight);

  function submitInput(): void {
    if (!operatorInputValid) {
      localValidationMessage = 'Operator input must be valid JSON.';
      return;
    }
    localValidationMessage = null;
    void onSubmitInput?.(JSON.parse(operatorInput));
  }

  function reject(): void {
    const reason = rejectReason.trim();
    if (!reason) {
      localValidationMessage = 'A rejection reason is required.';
      return;
    }
    localValidationMessage = null;
    void onReject?.(reason);
  }

  function isValidJson(value: string): boolean {
    try {
      JSON.parse(value);
      return true;
    } catch {
      return false;
    }
  }
</script>

<section class="border-y border-admin-border bg-admin-panel py-5" data-testid="workflow-run-detail-controls">
  <div class="grid gap-5 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
    <div class="min-w-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Operations</p>
      <h2 class="m-0 mt-1 text-base font-semibold text-admin-ink">Run controls</h2>
      <p class="m-0 mt-2 text-sm leading-6 text-admin-muted" data-testid="workflow-run-control-cancel-reason">
        {availability.cancelReason}
      </p>
      <div class="mt-4 flex flex-wrap gap-2">
        <button
          class="inline-flex h-10 items-center gap-2 rounded-md border border-red-200 bg-white px-3 text-sm font-medium text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50"
          type="button"
          disabled={!availability.canCancel || actionInFlight}
          onclick={() => void onCancel?.()}
          data-testid="workflow-run-detail-cancel"
        >
          <Ban size={16} strokeWidth={1.9} aria-hidden="true" />
          <span>Cancel run</span>
        </button>
      </div>
    </div>

    <div class="min-w-0 border-t border-admin-border pt-5 lg:border-l lg:border-t-0 lg:pl-5 lg:pt-0">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Intervention</p>
      <h3 class="m-0 mt-1 text-sm font-semibold text-admin-ink">
        {pendingRequest?.kind ?? 'No operator action pending'}
      </h3>
      <p class="m-0 mt-2 text-sm leading-6 text-admin-muted" data-testid="workflow-run-detail-pending-prompt">
        {pendingRequest?.prompt ?? availability.interventionReason}
      </p>

      <div class="mt-4 grid gap-4 md:grid-cols-2">
        <label class="grid min-w-0 gap-1.5 text-sm font-medium text-admin-ink">
          Operator input JSON
          <textarea
            class="min-h-28 w-full resize-y rounded-md border border-admin-border bg-white p-3 font-mono text-xs leading-5 text-admin-ink outline-none focus:border-admin-accent focus:ring-2 focus:ring-admin-accent/20 disabled:bg-admin-soft"
            bind:value={operatorInput}
            disabled={!canResolve}
            aria-invalid={!operatorInputValid}
            aria-describedby="workflow-run-operator-input-help"
            data-testid="workflow-run-detail-operator-input"
          ></textarea>
          <span
            id="workflow-run-operator-input-help"
            class:text-red-700={!operatorInputValid}
            class:text-admin-muted={operatorInputValid}
            class="text-xs"
            data-testid="workflow-run-detail-operator-input-help"
          >
            {operatorInputValid
              ? 'A JSON value passed to the waiting workflow step.'
              : 'Enter valid JSON before submitting operator input.'}
          </span>
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm font-medium text-admin-ink">
          Rejection reason
          <textarea
            class="min-h-28 w-full resize-y rounded-md border border-admin-border bg-white p-3 text-sm leading-5 text-admin-ink outline-none focus:border-admin-accent focus:ring-2 focus:ring-admin-accent/20 disabled:bg-admin-soft"
            bind:value={rejectReason}
            disabled={!canResolve}
            data-testid="workflow-run-detail-reject-reason"
          ></textarea>
          <span class="text-xs text-admin-muted">Required only when rejecting the pending request.</span>
        </label>
      </div>

      {#if localValidationMessage}
        <div class="mt-3" data-testid="workflow-run-detail-local-validation">
          <AdminMessage tone="error" density="compact" title="Input not ready" message={localValidationMessage} />
        </div>
      {/if}

      <div class="mt-4 flex flex-wrap gap-2">
        <button
          class="inline-flex h-10 items-center gap-2 rounded-md bg-admin-accent px-3 text-sm font-medium text-white hover:brightness-95 disabled:cursor-not-allowed disabled:opacity-50"
          type="button"
          disabled={!canResolve || !operatorInputValid}
          onclick={submitInput}
          data-testid="workflow-run-detail-submit-input"
        >
          <Send size={16} strokeWidth={1.9} aria-hidden="true" />
          <span>Submit input</span>
        </button>
        <button
          class="inline-flex h-10 items-center gap-2 rounded-md border border-admin-border bg-white px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-50"
          type="button"
          disabled={!canResolve}
          onclick={() => void onResume?.()}
          data-testid="workflow-run-detail-resume"
        >
          <Play size={16} strokeWidth={1.9} aria-hidden="true" />
          <span>Resume without input</span>
        </button>
        <button
          class="inline-flex h-10 items-center gap-2 rounded-md border border-red-200 bg-white px-3 text-sm font-medium text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50"
          type="button"
          disabled={!canResolve || rejectReason.trim().length === 0}
          onclick={reject}
          data-testid="workflow-run-detail-reject"
        >
          <XCircle size={16} strokeWidth={1.9} aria-hidden="true" />
          <span>Reject request</span>
        </button>
      </div>
    </div>
  </div>

  <div class="mt-4">
    <ActionFeedback
      state={actionState}
      successTitle="Workflow run updated"
      errorTitle="Workflow run action failed"
      successTestId="workflow-run-detail-action-success"
      errorTestId="workflow-run-detail-action-error"
      runningTestId="workflow-run-detail-action-running"
    />
  </div>
</section>
