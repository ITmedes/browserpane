<script lang="ts">
  import { buildWorkflowEventDeliverySummary } from '$lib/workflow-events/workflow-event-view-model';
  import type { WorkflowEventDeliveryResource } from '$lib/workflow-events/workflow-event-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  let { deliveries }: { readonly deliveries: readonly WorkflowEventDeliveryResource[] } = $props();
  const model = $derived(buildWorkflowEventDeliverySummary(deliveries));
</script>

<section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Delivery health">
  {#each model.metrics as metric}<div
      class="rounded-md border border-admin-border bg-admin-soft/50 p-4"
      data-testid={metric.testId}
    >
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">{metric.label}</p>
      <p class="m-0 mt-2 text-2xl font-semibold text-admin-ink">{metric.value}</p>
    </div>{/each}
</section>
<section
  class="rounded-md border border-admin-border bg-admin-soft/50 p-4"
  data-testid="workflow-event-deliveries"
>
  <div class="border-b border-admin-border pb-3">
    <h3 class="m-0 text-sm font-semibold text-admin-ink">Persisted delivery diagnostics</h3>
    <p class="m-0 mt-1 text-xs text-admin-muted">
      Status, retry timing, receiver response, and attempts for this subscription.
    </p>
  </div>
  {#if model.rows.length === 0}<p
      class="m-0 mt-4 rounded-md border border-dashed border-admin-border bg-admin-panel p-4 text-sm text-admin-muted"
      data-testid="workflow-event-deliveries-empty"
    >
      No deliveries have been persisted for this subscription.
    </p>{:else}<div class="mt-4 grid gap-3">
      {#each model.rows as row}<article
          class="rounded-md border border-admin-border bg-admin-panel p-4"
          data-testid="workflow-event-delivery-row"
        >
          <div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
            <div>
              <div class="flex flex-wrap items-center gap-2">
                <strong class="text-sm text-admin-ink">{row.eventType}</strong><span
                  class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.tone)}`}
                  >{row.state}</span
                ><span class="text-xs text-admin-muted">{row.attempts}</span>
              </div>
              <p class="m-0 mt-2 text-xs text-admin-muted">
                {row.response} · {row.error} · retry: {row.retry}
              </p>
              <p class="m-0 mt-1 font-mono text-[11px] text-admin-muted">
                run {row.runId} · event {row.eventId}
              </p>
            </div>
            <span class="text-xs text-admin-muted">{row.updatedAt}</span>
          </div>
          <details
            class="mt-3 border-t border-admin-border pt-3"
            data-testid="workflow-event-delivery-details"
          >
            <summary class="cursor-pointer text-xs font-semibold text-admin-ink"
              >Attempts and payload</summary
            >
            <div class="mt-3 grid gap-3">
              <div class="grid gap-2">
                {#each row.attemptsDetail as attempt}<div
                    class="rounded border border-admin-border bg-admin-soft p-2 text-xs text-admin-muted"
                  >
                    <strong class="text-admin-ink">{attempt.label}</strong> · {attempt.response} ·
                    {attempt.error} · {attempt.createdAt}
                  </div>{/each}
              </div>
              <pre
                class="max-h-72 overflow-auto rounded-md bg-admin-night p-3 text-xs text-white">{row.payloadText}</pre>
            </div>
          </details>
        </article>{/each}
    </div>{/if}
</section>
