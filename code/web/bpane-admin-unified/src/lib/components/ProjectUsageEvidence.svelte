<script lang="ts">
  import type { ProjectUsagePressureState } from '$lib/projects/project-governance-types';
  import { ProjectUsagePresenter } from '$lib/projects/project-usage-presenter';
  import type { ProjectResource } from '$lib/projects/project-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';

  type ProjectUsageEvidenceProps = {
    readonly project: ProjectResource;
  };

  let { project }: ProjectUsageEvidenceProps = $props();
  const presenter = new ProjectUsagePresenter();
  const model = $derived(presenter.build(project));

  function stateLabel(state: ProjectUsagePressureState): string {
    switch (state) {
      case 'unbounded': return 'No limit';
      case 'normal': return 'Within limit';
      case 'approaching': return 'Approaching';
      case 'at_limit': return 'At limit';
      case 'exceeded': return 'Exceeded';
    }
  }
</script>

<section class="border-b border-admin-border px-4 py-5 sm:px-5" data-testid="project-usage-evidence">
  <div class="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
    <div>
      <h4 class="m-0 text-sm font-semibold text-admin-ink">Usage and limits</h4>
      <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
        Observed {model.observedAt}. Capacity pressure can queue work; alerts alone are not denials.
      </p>
    </div>
    <span class="inline-flex w-fit rounded-full bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-700">
      {model.enforcement}
    </span>
  </div>

  <p class="m-0 mt-3 text-sm text-admin-ink" data-testid="project-budget-enforcement">
    {model.enforcementLabel}
  </p>

  <dl class="mt-4 grid gap-x-5 gap-y-4 sm:grid-cols-2 xl:grid-cols-3">
    {#each model.metrics as metric}
      <div class="min-w-0 border-l-2 border-admin-border pl-3" data-testid={`project-usage-${metric.id}`}>
        <div class="flex min-w-0 items-start justify-between gap-2">
          <dt class="text-xs font-semibold uppercase text-admin-muted">{metric.label}</dt>
          <span class={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-semibold ring-1 ${projectToneClass(metric.tone)}`}>
            {stateLabel(metric.state)}
          </span>
        </div>
        <dd class="m-0 mt-1 break-words text-sm font-semibold text-admin-ink">{metric.displayValue}</dd>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{metric.description}</p>
        {#if metric.id === 'egress_total_bytes'}
          <p class="m-0 mt-1 text-xs font-medium text-admin-muted" data-testid="project-egress-breakdown">
            RX {model.egressReceiveLabel} / TX {model.egressTransmitLabel}
          </p>
        {/if}
      </div>
    {/each}
  </dl>

  <div class="mt-4 grid gap-2" data-testid="project-governance-alerts">
    {#if model.alerts.length === 0}
      <AdminMessage tone="info" density="compact" title="Usage alerts" message="No generated alerts." />
    {:else}
      {#each model.alerts as alert}
        <AdminMessage
          tone="warning"
          density="compact"
          title={`${alert.metric} ${alert.state}`}
          message={alert.message}
        />
      {/each}
    {/if}
  </div>
</section>
