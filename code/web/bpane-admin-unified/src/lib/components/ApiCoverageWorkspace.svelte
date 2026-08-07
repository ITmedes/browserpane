<script lang="ts">
  import { onMount } from 'svelte';
  import { CheckCircle2, Download, Search } from '@lucide/svelte';
  import {
    API_AUTH_MODES,
    API_CLASSIFICATIONS,
    type ApiAuthMode,
    type ApiClassification,
    type ApiContractEvidence,
    type ApiOperation,
  } from '$lib/api-companion/api-contract-types';
  import {
    AUTH_DEFINITIONS,
    CLASSIFICATION_DEFINITIONS,
    authDefinition,
    classificationDefinition,
    filterApiOperations,
    operationFamilies,
  } from '$lib/api-companion/api-companion-view-model';
  import AdminMessage from './AdminMessage.svelte';
  import ApiContractSummary from './ApiContractSummary.svelte';

  type ApiCoverageWorkspaceProps = {
    readonly evidence: ApiContractEvidence;
  };

  let { evidence }: ApiCoverageWorkspaceProps = $props();
  let query = $state('');
  let classification = $state<ApiClassification | 'all'>('all');
  let auth = $state<ApiAuthMode | 'all'>('all');
  let family = $state<string>('all');

  const families = $derived(operationFamilies(evidence.operations));
  const filteredOperations = $derived(filterApiOperations(evidence.operations, { query, classification, auth, family }));

  onMount(() => {
    query = new URLSearchParams(window.location.search).get('operation') ?? '';
  });

  function methodClass(method: ApiOperation['method']): string {
    switch (method) {
      case 'GET': return 'bg-emerald-50 text-emerald-800 ring-emerald-200';
      case 'POST': return 'bg-blue-50 text-blue-800 ring-blue-200';
      case 'PUT': return 'bg-amber-50 text-amber-800 ring-amber-200';
      case 'DELETE': return 'bg-rose-50 text-rose-800 ring-rose-200';
    }
  }
</script>

<div class="mx-auto grid min-w-0 w-full max-w-[1540px] gap-6 px-4 py-6 sm:px-6" data-testid="api-coverage-workspace">
  <header class="flex min-w-0 flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
    <div class="min-w-0 max-w-4xl">
      <p class="m-0 text-xs font-semibold uppercase text-admin-muted">Docs / API coverage</p>
      <h1 class="m-0 mt-1 text-2xl font-bold text-admin-ink">Control API coverage</h1>
      <p class="m-0 mt-2 text-sm leading-6 text-admin-muted">
        Every frozen operation is assigned exactly one operator, evidence, companion, or worker ownership class.
      </p>
    </div>
    <a class="inline-flex h-9 w-fit items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-semibold text-admin-ink hover:bg-admin-soft" href="/openapi/bpane-control-v1.operations.json" download data-testid="api-download-operations">
      <Download size={16} strokeWidth={2} />
      Download inventory
    </a>
  </header>

  <ApiContractSummary operations={evidence.operations} />

  <AdminMessage
    tone="success"
    title="Classification integrity verified"
    message={`${evidence.operations.length} operations match the independent classification catalog. Repository CI separately enforces OpenAPI lint, route recognition, examples, and semantic compatibility.`}
    testId="api-coverage-integrity"
  />

  <section class="grid min-w-0 gap-4 rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm" aria-label="Coverage filters" data-testid="api-coverage-filters">
    <div class="grid min-w-0 gap-3 md:grid-cols-2 xl:grid-cols-[minmax(15rem,1.6fr)_repeat(3,minmax(11rem,1fr))]">
      <label class="grid min-w-0 gap-1 text-xs font-semibold text-admin-muted">
        Search operations
        <span class="flex h-10 min-w-0 items-center gap-2 rounded-md border border-admin-border bg-admin-paper px-3 focus-within:border-admin-accent focus-within:ring-2 focus-within:ring-admin-accent/20">
          <Search size={16} strokeWidth={2} class="shrink-0" />
          <input class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none" type="search" bind:value={query} placeholder="Operation, method, path, response" data-testid="api-coverage-search" />
        </span>
      </label>
      <label class="grid min-w-0 gap-1 text-xs font-semibold text-admin-muted">
        Classification
        <select class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-paper px-3 text-sm text-admin-ink" bind:value={classification} data-testid="api-coverage-classification">
          <option value="all">All classifications</option>
          {#each API_CLASSIFICATIONS as value}
            <option value={value}>{classificationDefinition(value).shortLabel}</option>
          {/each}
        </select>
      </label>
      <label class="grid min-w-0 gap-1 text-xs font-semibold text-admin-muted">
        Authentication
        <select class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-paper px-3 text-sm text-admin-ink" bind:value={auth} data-testid="api-coverage-auth">
          <option value="all">All auth modes</option>
          {#each API_AUTH_MODES as value}
            <option value={value}>{authDefinition(value).label}</option>
          {/each}
        </select>
      </label>
      <label class="grid min-w-0 gap-1 text-xs font-semibold text-admin-muted">
        API family
        <select class="h-10 min-w-0 rounded-md border border-admin-border bg-admin-paper px-3 text-sm text-admin-ink" bind:value={family} data-testid="api-coverage-family">
          <option value="all">All families</option>
          {#each families as value}
            <option value={value}>{value}</option>
          {/each}
        </select>
      </label>
    </div>

    <div class="flex min-w-0 flex-wrap items-center justify-between gap-3 border-t border-admin-border pt-3 text-xs text-admin-muted">
      <span data-testid="api-coverage-result-count">{filteredOperations.length} of {evidence.operations.length} operations</span>
      <button type="button" class="font-semibold text-admin-accent hover:underline" onclick={() => { query = ''; classification = 'all'; auth = 'all'; family = 'all'; }} data-testid="api-coverage-clear">Clear filters</button>
    </div>
  </section>

  <section class="min-w-0 overflow-hidden rounded-md border border-admin-border bg-admin-panel shadow-sm" aria-label="Operation inventory">
    {#if filteredOperations.length === 0}
      <div class="p-4">
        <AdminMessage tone="info" title="No operations match" message="Clear one or more filters to return to the complete contract inventory." testId="api-coverage-empty" />
      </div>
    {:else}
      <div class="max-w-full overflow-x-auto">
        <table class="w-full min-w-[1080px] border-collapse text-left text-sm" data-testid="api-coverage-table">
          <thead class="bg-admin-soft text-xs font-semibold uppercase text-admin-muted">
            <tr>
              <th class="px-4 py-3">Family / operation</th>
              <th class="px-4 py-3">Request</th>
              <th class="px-4 py-3">Authentication</th>
              <th class="px-4 py-3">Ownership</th>
              <th class="px-4 py-3">Responses</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-admin-border">
            {#each filteredOperations as operation (operation.operationId)}
              <tr class="align-top hover:bg-admin-soft/50" data-testid="api-operation-row" data-operation-id={operation.operationId}>
                <td class="px-4 py-3">
                  <span class="block text-xs font-semibold text-admin-muted">{operation.tags.join(', ')}</span>
                  <strong class="mt-1 block font-mono text-xs text-admin-ink">{operation.operationId}</strong>
                </td>
                <td class="px-4 py-3">
                  <div class="flex min-w-0 items-start gap-2">
                    <span class={`inline-flex shrink-0 rounded-full px-2 py-0.5 text-xs font-bold ring-1 ${methodClass(operation.method)}`}>{operation.method}</span>
                    <code class="min-w-0 break-all text-xs leading-5 text-admin-ink">{operation.path}</code>
                  </div>
                </td>
                <td class="px-4 py-3 text-xs text-admin-muted">{authDefinition(operation.auth).label}</td>
                <td class="px-4 py-3">
                  <span class="inline-flex rounded-full bg-admin-soft px-2 py-0.5 text-xs font-semibold text-admin-ink ring-1 ring-admin-border">{classificationDefinition(operation.classification).shortLabel}</span>
                </td>
                <td class="px-4 py-3 text-xs text-admin-muted">{operation.responses.join(', ')}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </section>

  <section class="grid min-w-0 gap-4 md:grid-cols-2 xl:grid-cols-4" aria-label="Classification definitions" data-testid="api-classification-definitions">
    {#each CLASSIFICATION_DEFINITIONS as definition (definition.id)}
      <article class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm">
        <div class="flex items-start gap-2">
          <CheckCircle2 size={17} strokeWidth={2} class="mt-0.5 shrink-0 text-admin-accent" />
          <div class="min-w-0">
            <h2 class="m-0 text-sm font-semibold text-admin-ink">{definition.label}</h2>
            <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{definition.description}</p>
          </div>
        </div>
      </article>
    {/each}
  </section>

  <section class="grid min-w-0 gap-4 md:grid-cols-3" aria-label="Authentication definitions" data-testid="api-auth-definitions">
    {#each AUTH_DEFINITIONS as definition (definition.id)}
      <article class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-4 shadow-sm">
        <h2 class="m-0 text-sm font-semibold text-admin-ink">{definition.label}</h2>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">{definition.description}</p>
      </article>
    {/each}
  </section>
</div>
