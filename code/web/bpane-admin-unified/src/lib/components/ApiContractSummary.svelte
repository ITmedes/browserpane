<script lang="ts">
  import {
    classificationSummaries,
    type ApiSummaryItem,
  } from '$lib/api-companion/api-companion-view-model';
  import type { ApiOperation } from '$lib/api-companion/api-contract-types';

  type ApiContractSummaryProps = {
    readonly operations: readonly ApiOperation[];
    readonly compact?: boolean;
  };

  let { operations, compact = false }: ApiContractSummaryProps = $props();
  const summaries = $derived(classificationSummaries(operations));

  function testId(item: ApiSummaryItem): string {
    return `api-summary-${item.id}`;
  }
</script>

<section
  class="grid min-w-0 grid-cols-2 gap-px overflow-hidden rounded-md border border-admin-border bg-admin-border lg:grid-cols-5"
  aria-label="API contract summary"
  data-testid="api-contract-summary"
>
  <div class={compact ? 'bg-admin-panel p-3' : 'bg-admin-panel p-4'} data-testid="api-summary-total">
    <span class="block text-xs font-semibold uppercase text-admin-muted">Frozen operations</span>
    <strong class="mt-1 block text-xl font-bold text-admin-ink">{operations.length}</strong>
  </div>
  {#each summaries as item (item.id)}
    <div class={compact ? 'bg-admin-panel p-3' : 'bg-admin-panel p-4'} data-testid={testId(item)}>
      <span class="block text-xs font-semibold uppercase text-admin-muted">{item.label}</span>
      <strong class="mt-1 block text-xl font-bold text-admin-ink">{item.count}</strong>
    </div>
  {/each}
</section>
