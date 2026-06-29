<script lang="ts">
  import type { BrowserContextStatusSummaryModel } from '$lib/browser-contexts/browser-context-edit-view-model';
  import { projectToneClass } from '$lib/projects/project-ui';

  type BrowserContextStatusSummaryProps = {
    readonly model: BrowserContextStatusSummaryModel;
  };

  let { model }: BrowserContextStatusSummaryProps = $props();
</script>

<section class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="browser-context-status-summary">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-3 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h4 class="m-0 text-sm font-semibold text-admin-ink">Current status</h4>
      <p class="m-0 mt-1 truncate font-mono text-xs text-admin-muted">{model.contextId}</p>
    </div>
    <div class="flex min-w-0 flex-wrap gap-2">
      <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.stateTone)}`}>
        {model.stateLabel}
      </span>
      <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.persistenceTone)}`}>
        {model.persistenceLabel}
      </span>
    </div>
  </div>

  <dl class="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-5">
    {#each model.items as item}
      <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
        <dt class="text-xs font-semibold uppercase text-admin-muted">{item.label}</dt>
        <dd class={`m-0 mt-1 inline-flex max-w-full rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(item.tone)}`}>
          {item.value}
        </dd>
      </div>
    {/each}
  </dl>
</section>
