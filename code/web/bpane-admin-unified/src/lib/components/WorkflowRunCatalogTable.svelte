<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildWorkflowRunOverviewModel,
    isActiveWorkflowRun,
    runNeedsAttention,
    workflowRunMatchesSearch,
    type WorkflowRunOverviewRow,
  } from '$lib/workflow-runs/workflow-run-overview-view-model';
  import type { WorkflowRunResource } from '$lib/workflow-runs/workflow-run-types';
  import { workflowRunDetailHref } from '$lib/workflow-runs/workflow-run-detail-view-model';
  import { projectToneClass } from '$lib/projects/project-ui';

  type WorkflowRunLens = 'all' | 'active' | 'awaiting_input' | 'failed' | 'completed' | 'attention';

  type WorkflowRunLensDefinition = {
    readonly id: WorkflowRunLens;
    readonly label: string;
    readonly count: number;
    readonly showDot?: boolean;
  };

  type WorkflowRunTableItem = {
    readonly run: WorkflowRunResource;
    readonly row: WorkflowRunOverviewRow;
  };

  type WorkflowRunCatalogTableProps = {
    readonly runs: readonly WorkflowRunResource[];
  };

  let { runs }: WorkflowRunCatalogTableProps = $props();
  let workflowRunLens = $state<WorkflowRunLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildWorkflowRunOverviewModel(runs));
  const items = $derived(runs.map((run, index) => ({ run, row: model.rows[index]! })));
  const lensDefinitions = $derived(buildLensDefinitions(items));
  const visibleItems = $derived(filterItems(items, workflowRunLens, searchQuery));

  function buildLensDefinitions(items: readonly WorkflowRunTableItem[]): readonly WorkflowRunLensDefinition[] {
    return [
      { id: 'all', label: 'All', count: items.length },
      {
        id: 'active',
        label: 'Active',
        count: items.filter((item) => isActiveWorkflowRun(item.run)).length,
        showDot: true,
      },
      {
        id: 'awaiting_input',
        label: 'Awaiting input',
        count: items.filter((item) => item.run.state === 'awaiting_input').length,
      },
      {
        id: 'failed',
        label: 'Failed',
        count: items.filter((item) => item.run.state === 'failed' || item.run.state === 'timed_out').length,
      },
      {
        id: 'completed',
        label: 'Completed',
        count: items.filter((item) => ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(item.run.state)).length,
      },
      { id: 'attention', label: 'Needs attention', count: items.filter((item) => runNeedsAttention(item.run)).length },
    ];
  }

  function filterItems(
    items: readonly WorkflowRunTableItem[],
    lens: WorkflowRunLens,
    query: string,
  ): readonly WorkflowRunTableItem[] {
    const normalizedQuery = query.trim().toLowerCase();
    return items.filter((item) => matchesLens(item, lens) && workflowRunMatchesSearch(item.row, normalizedQuery));
  }

  function matchesLens(item: WorkflowRunTableItem, lens: WorkflowRunLens): boolean {
    if (lens === 'active') {
      return isActiveWorkflowRun(item.run);
    }
    if (lens === 'awaiting_input') {
      return item.run.state === 'awaiting_input';
    }
    if (lens === 'failed') {
      return item.run.state === 'failed' || item.run.state === 'timed_out';
    }
    if (lens === 'completed') {
      return ['succeeded', 'failed', 'cancelled', 'timed_out'].includes(item.run.state);
    }
    if (lens === 'attention') {
      return runNeedsAttention(item.run);
    }
    return true;
  }

  function workflowHref(workflowId: string): string {
    return `/admin-new/workflows/${encodeURIComponent(workflowId)}`;
  }

  function sessionHref(sessionId: string): string {
    return `/admin-new/sessions/${encodeURIComponent(sessionId)}`;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="workflow-runs-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0">
      <h2 class="m-0 text-sm font-semibold text-admin-ink">Run catalog</h2>
      <p class="m-0 mt-1 text-xs text-admin-muted">Execution state, linked session, project admission, runtime hold, and produced output.</p>
    </div>
    <span class="text-xs text-admin-muted" data-testid="workflow-runs-list-count">
      {visibleItems.length} of {model.rows.length}
    </span>
  </div>

  <div class="flex flex-col gap-0 border-b border-admin-border bg-admin-panel lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center gap-0 px-4">
      {#each lensDefinitions as lens}
        <button
          class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${
            workflowRunLens === lens.id
              ? 'border-admin-accent text-admin-ink'
              : 'border-transparent text-admin-muted hover:text-admin-ink'
          }`}
          type="button"
          aria-pressed={workflowRunLens === lens.id}
          onclick={() => {
            workflowRunLens = lens.id;
          }}
          data-testid={`workflow-runs-lens-${lens.id}`}
        >
          {#if lens.showDot}
            <span class="h-1.5 w-1.5 rounded-full bg-admin-success" aria-hidden="true"></span>
          {/if}
          <span>{lens.label}</span>
          <span class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted">
            {lens.count}
          </span>
        </button>
      {/each}
    </div>

    <label class="mx-4 mb-3 flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[360px]">
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Search workflow runs</span>
      <input
        class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted"
        type="search"
        placeholder="Run, workflow, session, state..."
        bind:value={searchQuery}
        data-testid="workflow-runs-search"
      />
    </label>
  </div>

  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto bg-admin-panel">
    <table class="w-full min-w-[1260px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft">
        <tr class="border-b border-admin-border">
          <th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Run</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">State</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Workflow</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Session</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Project</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Runtime</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Output</th>
          <th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted" scope="col">Updated</th>
          <th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#if visibleItems.length === 0}
          <tr>
            <td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="9" data-testid="workflow-runs-filter-empty">
              No workflow runs match the current filters.
            </td>
          </tr>
        {:else}
          {#each visibleItems as item}
            {@const row = item.row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="workflow-runs-list-row">
              <td class="w-[230px] px-4 py-3 align-middle">
                <div class="grid min-w-0 text-left">
                  <a
                    class="truncate font-mono text-sm font-semibold text-admin-ink hover:text-admin-accent"
                    href={workflowRunDetailHref(row.id)}
                    title={row.id}
                    data-testid="workflow-runs-detail-link"
                  >{row.shortId}</a>
                  <span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span>
                  <span class="mt-1 truncate text-xs text-admin-muted">{row.source}</span>
                </div>
              </td>
              <td class="px-3 py-3 align-middle">
                <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>
                  {row.state}
                </span>
                <span class="mt-1 block max-w-[180px] truncate text-xs text-admin-muted">{row.intervention}</span>
              </td>
              <td class="max-w-[220px] px-3 py-3 align-middle text-xs text-admin-muted">
                <a
                  class="block truncate text-sm font-semibold text-admin-ink hover:text-admin-accent"
                  href={workflowHref(row.workflowId)}
                  data-testid="workflow-runs-workflow-link"
                >
                  {row.workflowId}
                </a>
                <span class="mt-1 block truncate">version {row.workflowVersion}</span>
              </td>
              <td class="max-w-[210px] px-3 py-3 align-middle text-xs text-admin-muted">
                <a
                  class="block truncate text-sm font-semibold text-admin-ink hover:text-admin-accent"
                  href={sessionHref(row.sessionId)}
                  data-testid="workflow-runs-session-link"
                >
                  {row.shortSessionId}
                </a>
                <span class="mt-1 block truncate font-mono">{row.sessionId}</span>
              </td>
              <td class="max-w-[210px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="block truncate font-semibold text-admin-ink">{row.project}</span>
                <span class="mt-1 block truncate">{row.admission}</span>
              </td>
              <td class="max-w-[230px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="line-clamp-2">{row.runtime}</span>
              </td>
              <td class="max-w-[190px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="block truncate">{row.output}</span>
                <span class="mt-1 block truncate">{row.error}</span>
              </td>
              <td class="max-w-[190px] px-3 py-3 align-middle text-xs text-admin-muted">
                <span class="block">{row.updatedAt}</span>
                <span class="mt-1 block truncate">{row.terminalAt}</span>
              </td>
              <td class="px-4 py-3 align-middle text-right">
                <div class="flex justify-end gap-2">
                  <a
                    class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                    href={workflowRunDetailHref(row.id)}
                    data-testid="workflow-runs-open-detail"
                  >
                    Details
                  </a>
                  <a
                    class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                    href={sessionHref(row.sessionId)}
                    data-testid="workflow-runs-open-session"
                  >
                    Session
                  </a>
                  <a
                    class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft"
                    href={workflowHref(row.workflowId)}
                    data-testid="workflow-runs-open-workflow"
                  >
                    Workflow
                  </a>
                </div>
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</section>
