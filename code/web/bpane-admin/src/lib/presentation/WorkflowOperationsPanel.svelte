<script lang="ts">
  import type { WorkflowOperationsViewModel } from './workflow-operations-view-model';

  type WorkflowOperationsPanelProps = {
    readonly viewModel: WorkflowOperationsViewModel;
  };

  let { viewModel }: WorkflowOperationsPanelProps = $props();
</script>

<section class="grid gap-4" aria-label="Workflow operations">
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div>
      <p class="admin-eyebrow">Workflows</p>
      <h2 class="m-0 text-base font-extrabold text-admin-night">{viewModel.title}</h2>
    </div>
    <span class="rounded-full bg-admin-warm/12 px-3 py-1 text-xs font-extrabold text-admin-warm" data-testid="workflow-status">
      {viewModel.status}
    </span>
  </div>

  <p class="m-0 text-sm leading-normal text-admin-ink/68">{viewModel.note}</p>

  <div class="grid gap-3">
    <label class="grid gap-1.5 text-sm font-bold text-admin-ink">
      Definition
      <select class="min-h-10 rounded-[14px] border border-admin-ink/14 bg-admin-cream px-3 text-admin-ink" disabled>
        <option>Workflow definition loading pending</option>
      </select>
    </label>
    <label class="grid gap-1.5 text-sm font-bold text-admin-ink">
      Run input
      <textarea class="min-h-24 rounded-[14px] border border-admin-ink/14 bg-admin-cream p-3 font-mono text-xs text-admin-ink" disabled>{'{}'}</textarea>
    </label>
  </div>

  <div class="grid grid-cols-2 gap-2 max-[760px]:grid-cols-1">
    <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
      Session
      <strong class="mt-1 block font-mono text-admin-ink normal-case">{viewModel.selectedSessionLabel}</strong>
    </span>
    <span class="rounded-[14px] bg-admin-leaf/10 p-3 text-xs font-bold text-admin-ink/68 uppercase">
      Output
      <strong class="mt-1 block text-admin-ink normal-case">logs and artifacts pending</strong>
    </span>
  </div>

  <div class="flex flex-wrap gap-2">
    <button class="admin-button-primary" type="button" disabled={!viewModel.canRefresh}>Refresh definitions</button>
    <button class="admin-button-primary" type="button" disabled={!viewModel.canRun}>Invoke run</button>
    <button class="admin-button-primary" type="button" disabled={!viewModel.canReleaseHold}>Release hold</button>
    <button class="admin-button-primary" type="button" disabled={!viewModel.canCancel}>Cancel run</button>
  </div>
</section>
