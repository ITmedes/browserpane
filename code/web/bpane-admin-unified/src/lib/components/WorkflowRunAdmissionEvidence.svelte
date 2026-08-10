<script lang="ts">
  import { projectToneClass } from '$lib/projects/project-ui';
  import type { WorkflowRunAdmissionEvidence as AdmissionEvidence } from '$lib/workflow-runs/workflow-run-detail-view-model';
  import AdminMessage from './AdminMessage.svelte';

  type WorkflowRunAdmissionEvidenceProps = {
    readonly evidence: AdmissionEvidence;
  };

  let { evidence }: WorkflowRunAdmissionEvidenceProps = $props();
</script>

<section class="grid min-w-0 gap-4 border-t border-admin-border pt-4" data-testid="workflow-run-admission-evidence">
  <div class="min-w-0">
    <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Project admission</p>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
      Authoritative gateway evidence for this run, evaluated at the check time shown below.
    </p>
  </div>

  {#if evidence.state}
    <div class="grid min-w-0 gap-3">
      <div class="flex min-w-0 flex-wrap items-center gap-2">
        <span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(evidence.tone)}`} data-testid="workflow-run-admission-state">
          {evidence.state}
        </span>
        {#if evidence.reasonCode}
          <code class="min-w-0 break-all text-xs text-admin-muted" data-testid="workflow-run-admission-reason-code">{evidence.reasonCode}</code>
        {/if}
      </div>
      {#if evidence.message}
        <p class="m-0 text-sm leading-6 text-admin-ink" data-testid="workflow-run-admission-message">{evidence.message}</p>
      {/if}
      {#if evidence.checkedAt}
        <p class="m-0 text-xs text-admin-muted" data-testid="workflow-run-admission-checked-at">Checked {evidence.checkedAt}</p>
      {/if}
      {#if evidence.facts.length > 0}
        <dl class="grid min-w-0 gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {#each evidence.facts as fact}
            <div class="min-w-0 border-l-2 border-admin-border px-3 py-1">
              <dt class="text-xs font-semibold uppercase text-admin-muted">{fact.label}</dt>
              <dd class="m-0 mt-1 break-words text-sm font-semibold text-admin-ink" data-testid={fact.testId}>{fact.value}</dd>
            </div>
          {/each}
        </dl>
      {/if}
    </div>
  {:else}
    <AdminMessage
      tone="empty"
      density="compact"
      title="No project admission snapshot"
      message="This owner-scoped or legacy run does not expose a project admission decision."
      testId="workflow-run-admission-empty"
    />
  {/if}

  {#if evidence.queueReason || evidence.queuedAt}
    <div class="rounded-md border border-amber-200 bg-amber-50 p-3" data-testid="workflow-run-queue-evidence">
      <p class="m-0 text-xs font-semibold uppercase text-amber-800">Queue evidence</p>
      <p class="m-0 mt-1 text-sm text-amber-950" data-testid="workflow-run-queue-reason">{evidence.queueReason ?? 'No queue reason provided.'}</p>
      {#if evidence.queuedAt}
        <p class="m-0 mt-1 text-xs text-amber-800" data-testid="workflow-run-queued-at">Queued {evidence.queuedAt}</p>
      {/if}
    </div>
  {/if}
</section>
