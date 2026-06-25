<script lang="ts">
  import type { ProjectInspectorModel, ProjectInspectorRow } from '$lib/projects/project-inspector-view-model';

  type ProjectStatusSummaryProps = {
    readonly model: ProjectInspectorModel;
  };

  let { model }: ProjectStatusSummaryProps = $props();

  type StatusSection = {
    readonly title: string;
    readonly rows: readonly ProjectInspectorRow[];
  };

  const sections = $derived<readonly StatusSection[]>([
    { title: 'Project identity', rows: model.identityRows },
    { title: 'Current usage', rows: model.usageRows },
  ]);
</script>

<section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="project-status-summary">
  <div class="border-b border-admin-border pb-3">
    <h4 class="m-0 text-sm font-semibold text-admin-ink">Current status</h4>
    <p class="m-0 mt-1 text-xs text-admin-muted">
      Immutable project identity, live usage counters, and generated usage alerts.
    </p>
  </div>

  <div class="mt-4 grid gap-5 xl:grid-cols-2">
    {#each sections as section}
      <section class="min-w-0" aria-label={section.title}>
        <h5 class="m-0 text-sm font-semibold text-admin-ink">{section.title}</h5>
        <dl class="mt-3 grid gap-2">
          {#each section.rows as row}
            <div class="grid gap-1">
              <dt class="text-xs font-semibold uppercase text-admin-muted">{row.label}</dt>
              <dd class="m-0 break-words text-sm text-admin-ink">{row.value}</dd>
            </div>
          {/each}
        </dl>
      </section>
    {/each}
  </div>

  <section class="mt-5 min-w-0" aria-label="Usage alerts">
    <h5 class="m-0 text-sm font-semibold text-admin-ink">Usage alerts</h5>
    <dl class="mt-3 grid gap-2">
      {#each model.alerts as alert}
        <div class="grid gap-1">
          <dt class="text-xs font-semibold uppercase text-admin-muted">{alert.label}</dt>
          <dd class="m-0 break-words text-sm text-admin-ink">{alert.value}</dd>
        </div>
      {/each}
    </dl>
  </section>
</section>
