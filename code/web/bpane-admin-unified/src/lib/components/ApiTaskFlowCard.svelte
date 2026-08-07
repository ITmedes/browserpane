<script lang="ts">
  import { ArrowUpRight } from '@lucide/svelte';
  import {
    authDefinition,
    classificationDefinition,
    type ApiTaskFlow,
  } from '$lib/api-companion/api-companion-view-model';
  import ApiCommandBlock from './ApiCommandBlock.svelte';

  type ApiTaskFlowCardProps = {
    readonly flow: ApiTaskFlow;
  };

  let { flow }: ApiTaskFlowCardProps = $props();
</script>

<article class="min-w-0 rounded-md border border-admin-border bg-admin-panel shadow-sm" data-testid={`api-task-${flow.id}`}>
  <header class="flex flex-col gap-3 border-b border-admin-border p-4 sm:flex-row sm:items-start sm:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-base font-semibold text-admin-ink">{flow.title}</h2>
      <p class="m-0 mt-1 max-w-3xl text-sm leading-6 text-admin-muted">{flow.description}</p>
    </div>
    <a
      class="inline-flex h-9 w-fit shrink-0 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft"
      href={flow.adminHref}
      data-testid={`api-task-${flow.id}-admin-link`}
    >
      Open operator view
      <ArrowUpRight size={15} strokeWidth={2} />
    </a>
  </header>

  <div class="grid min-w-0 gap-5 p-4">
    {#each flow.steps as step, index (step.id)}
      <section class="grid min-w-0 gap-3" data-testid={`api-task-step-${step.id}`}>
        <div class="flex min-w-0 flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
          <div class="min-w-0">
            <span class="text-xs font-semibold uppercase text-admin-muted">Step {index + 1}</span>
            <h3 class="m-0 mt-1 text-sm font-semibold text-admin-ink">{step.title}</h3>
          </div>
          <div class="flex min-w-0 flex-wrap items-center gap-2 text-xs">
            <span class="rounded-full bg-admin-soft px-2 py-1 font-bold text-admin-ink">{step.operation.method}</span>
            <span class="rounded-full bg-admin-soft px-2 py-1 font-semibold text-admin-muted">{authDefinition(step.operation.auth).label}</span>
            <a class="font-semibold text-admin-accent hover:underline" href={step.coverageHref}> {classificationDefinition(step.operation.classification).shortLabel}</a>
          </div>
        </div>
        <ApiCommandBlock command={step.command} label={step.operation.operationId} testId={`api-command-${step.id}`} />
      </section>
    {/each}
  </div>
</article>
