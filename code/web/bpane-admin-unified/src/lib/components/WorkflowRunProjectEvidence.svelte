<script lang="ts">
  import type { ProjectUsagePressure } from '$lib/projects/project-governance-types';
  import type { ProjectResource } from '$lib/projects/project-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import { ProjectUsagePresenter } from '$lib/projects/project-usage-presenter';
  import AdminMessage from './AdminMessage.svelte';

  type WorkflowRunProjectEvidenceProps = {
    readonly project: ProjectResource;
  };

  let { project }: WorkflowRunProjectEvidenceProps = $props();
  const presenter = new ProjectUsagePresenter();
  const usage = $derived(presenter.build(project));
  const activeWorkflowRuns = $derived(metric('active_workflow_runs'));
  const activeSessions = $derived(metric('active_sessions'));

  function metric(id: ProjectUsagePressure['id']): ProjectUsagePressure | null {
    return usage.metrics.find((candidate) => candidate.id === id) ?? null;
  }
</script>

<section class="mt-3 border-t border-admin-border pt-3" data-testid="workflow-run-project-governance">
  <div class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
    <div class="min-w-0">
      <div class="flex flex-wrap items-center gap-2">
        <h5 class="m-0 text-sm font-semibold text-admin-ink">{project.name}</h5>
        <span class={`rounded-full px-2 py-0.5 text-[11px] font-semibold ring-1 ${projectToneClass(project.state === 'active' ? 'success' : 'warning')}`}>
          {project.state}
        </span>
      </div>
      <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
        Gateway admission remains authoritative if project capacity or policy changes before launch.
      </p>
    </div>
    <a class="shrink-0 text-xs font-semibold text-admin-accent hover:underline" href={`/admin-new/projects/${encodeURIComponent(project.id)}`}>
      Project details
    </a>
  </div>

  <div class="mt-3 flex flex-wrap gap-x-5 gap-y-2 text-xs text-admin-muted">
    {#if activeWorkflowRuns}<span>Active runs: <strong class="text-admin-ink">{activeWorkflowRuns.displayValue}</strong></span>{/if}
    {#if activeSessions}<span>Active sessions: <strong class="text-admin-ink">{activeSessions.displayValue}</strong></span>{/if}
    <span>Budgets: <strong class="text-admin-ink">{usage.enforcement}</strong></span>
  </div>

  {#if usage.alerts.length > 0}
    <div class="mt-3 grid gap-2" data-testid="workflow-run-project-alerts">
      {#each usage.alerts as alert}
        <AdminMessage tone="warning" density="compact" title={`${alert.metric} ${alert.state}`} message={alert.message} />
      {/each}
    </div>
  {/if}
</section>
