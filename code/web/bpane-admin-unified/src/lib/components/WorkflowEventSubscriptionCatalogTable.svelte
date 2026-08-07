<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildWorkflowEventOverviewModel,
    workflowEventSubscriptionMatchesSearch,
  } from '$lib/workflow-events/workflow-event-view-model';
  import type { WorkflowEventSubscriptionResource } from '$lib/workflow-events/workflow-event-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  let { subscriptions }: { readonly subscriptions: readonly WorkflowEventSubscriptionResource[] } =
    $props();
  let searchQuery = $state('');
  let lens = $state<'all' | 'signed' | 'wildcard' | 'explicit'>('all');
  const model = $derived(buildWorkflowEventOverviewModel(subscriptions));
  const visibleRows = $derived(
    model.rows.filter(
      (row) =>
        matchesLens(row) &&
        workflowEventSubscriptionMatchesSearch(row, searchQuery.trim().toLowerCase()),
    ),
  );
  const lenses = $derived([
    { id: 'all' as const, label: 'All', count: model.rows.length },
    {
      id: 'signed' as const,
      label: 'Signed',
      count: model.rows.filter((row) => row.signing === 'Signing configured').length,
    },
    {
      id: 'wildcard' as const,
      label: 'Wildcard',
      count: model.rows.filter((row) =>
        row.eventTypes.split(', ').some((type) => type.endsWith('.*')),
      ).length,
    },
    {
      id: 'explicit' as const,
      label: 'Explicit',
      count: model.rows.filter(
        (row) => !row.eventTypes.split(', ').some((type) => type.endsWith('.*')),
      ).length,
    },
  ]);
  function matchesLens(row: (typeof model.rows)[number]) {
    if (lens === 'signed') return row.signing === 'Signing configured';
    if (lens === 'wildcard') return row.eventTypes.split(', ').some((type) => type.endsWith('.*'));
    if (lens === 'explicit') return !row.eventTypes.split(', ').some((type) => type.endsWith('.*'));
    return true;
  }
</script>

<section
  class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel"
  data-testid="workflow-event-subscriptions-list"
>
  <div
    class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between"
  >
    <div>
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Event subscription catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">
        Signed outbound workflow lifecycle notifications and their delivery endpoints.
      </p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="workflow-event-subscriptions-list-count"
      >{visibleRows.length} of {model.rows.length}</span
    >
  </div>
  <div
    class="flex flex-col border-b border-admin-border lg:flex-row lg:items-center lg:justify-between"
  >
    <div class="flex flex-wrap items-center px-4">
      {#each lenses as definition}<button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${lens === definition.id ? 'border-admin-accent text-admin-ink' : 'border-transparent text-admin-muted hover:text-admin-ink'}`}
          type="button"
          aria-pressed={lens === definition.id}
          onclick={() => {
            lens = definition.id;
          }}
          data-testid={`workflow-event-subscriptions-lens-${definition.id}`}
          ><span>{definition.label}</span><span
            class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted"
            >{definition.count}</span
          ></button
        >{/each}
    </div>
    <label
      class="mx-4 mb-3 flex h-9 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[340px]"
      ><Search size={15} strokeWidth={1.8} /><span class="sr-only"
        >Search workflow event subscriptions</span
      ><input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none"
        type="search"
        placeholder="Name, target, event type..."
        bind:value={searchQuery}
        data-testid="workflow-event-subscriptions-search"
      /></label
    >
  </div>
  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto">
    <table class="w-full min-w-[940px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft"
        ><tr class="border-b border-admin-border"
          ><th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted"
            >Subscription</th
          ><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Target</th
          ><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted"
            >Event filters</th
          ><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Signing</th
          ><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Updated</th
          ><th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted">Actions</th
          ></tr
        ></thead
      ><tbody>
        {#if visibleRows.length === 0}<tr
            ><td
              class="px-4 py-14 text-center text-sm text-admin-muted"
              colspan="6"
              data-testid="workflow-event-subscriptions-filter-empty"
              >No event subscriptions match the current filters.</td
            ></tr
          >{:else}{#each visibleRows as row}<tr
              class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft"
              data-testid="workflow-event-subscriptions-list-row"
              ><td class="w-[260px] px-4 py-3"
                ><div class="grid min-w-0">
                  <span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span><span
                    class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span
                  >
                </div></td
              ><td class="max-w-[260px] px-3 py-3"
                ><span class="line-clamp-2 break-all text-xs text-admin-muted">{row.targetUrl}</span
                ></td
              ><td class="max-w-[240px] px-3 py-3"
                ><span class="line-clamp-2 text-xs text-admin-muted">{row.eventTypes}</span></td
              ><td class="px-3 py-3"
                ><span
                  class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.signingTone)}`}
                  >{row.signing}</span
                ></td
              ><td class="px-3 py-3 text-xs text-admin-muted">{row.updatedAt}</td><td
                class="px-4 py-3 text-right"
                ><a
                  class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                  href={`/admin-new/workflow-event-subscriptions/${encodeURIComponent(row.id)}`}
                  data-testid="workflow-event-subscriptions-detail-link">Details</a
                ></td
              ></tr
            >{/each}{/if}
      </tbody>
    </table>
  </div>
</section>
