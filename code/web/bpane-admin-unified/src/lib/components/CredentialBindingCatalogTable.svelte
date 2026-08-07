<script lang="ts">
  import { Search } from '@lucide/svelte';
  import {
    buildCredentialBindingOverviewModel,
    credentialBindingMatchesSearch,
    type CredentialBindingOverviewRow,
  } from '$lib/credential-bindings/credential-binding-view-model';
  import type { CredentialBindingResource } from '$lib/credential-bindings/credential-binding-types';
  import { projectToneClass } from '$lib/projects/project-ui';

  type BindingLens = 'all' | 'owner' | 'project' | 'form' | 'cookie' | 'storage' | 'totp';
  let { bindings }: { readonly bindings: readonly CredentialBindingResource[] } = $props();
  let lens = $state<BindingLens>('all');
  let searchQuery = $state('');

  const model = $derived(buildCredentialBindingOverviewModel(bindings));
  const visibleRows = $derived(filterRows(model.rows, lens, searchQuery));
  const lenses = $derived([
    { id: 'all' as const, label: 'All', count: model.rows.length },
    { id: 'owner' as const, label: 'Owner', count: model.rows.filter((row) => row.scope === 'Owner scoped').length },
    { id: 'project' as const, label: 'Project', count: model.rows.filter((row) => row.scope !== 'Owner scoped').length },
    { id: 'form' as const, label: 'Form fill', count: model.rows.filter((row) => row.injectionMode === 'Form fill').length },
    { id: 'cookie' as const, label: 'Cookie', count: model.rows.filter((row) => row.injectionMode === 'Cookie seed').length },
    { id: 'storage' as const, label: 'Storage', count: model.rows.filter((row) => row.injectionMode === 'Storage seed').length },
    { id: 'totp' as const, label: 'TOTP', count: model.rows.filter((row) => row.injectionMode === 'TOTP fill').length },
  ]);

  function filterRows(rows: readonly CredentialBindingOverviewRow[], selectedLens: BindingLens, query: string) {
    const normalizedQuery = query.trim().toLowerCase();
    return rows.filter((row) => matchesLens(row, selectedLens) && credentialBindingMatchesSearch(row, normalizedQuery));
  }

  function matchesLens(row: CredentialBindingOverviewRow, selectedLens: BindingLens): boolean {
    if (selectedLens === 'owner') return row.scope === 'Owner scoped';
    if (selectedLens === 'project') return row.scope !== 'Owner scoped';
    if (selectedLens === 'form') return row.injectionMode === 'Form fill';
    if (selectedLens === 'cookie') return row.injectionMode === 'Cookie seed';
    if (selectedLens === 'storage') return row.injectionMode === 'Storage seed';
    if (selectedLens === 'totp') return row.injectionMode === 'TOTP fill';
    return true;
  }
</script>

<section class="min-h-0 min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="credential-bindings-list">
  <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
    <div class="min-w-0"><h2 class="m-0 text-sm font-semibold text-admin-ink">Credential binding catalog</h2><p class="m-0 mt-1 text-xs text-admin-muted">Write-only secret references available to approved workflow and egress integrations.</p></div>
    <span class="text-xs text-admin-muted" data-testid="credential-bindings-list-count">{visibleRows.length} of {model.rows.length}</span>
  </div>
  <div class="flex flex-col border-b border-admin-border lg:flex-row lg:items-center lg:justify-between">
    <div class="flex min-w-0 flex-wrap items-center px-4">
      {#each lenses as definition}
        <button class={`inline-flex h-10 items-center gap-2 border-b-2 px-3 text-sm font-medium ${lens === definition.id ? 'border-admin-accent text-admin-ink' : 'border-transparent text-admin-muted hover:text-admin-ink'}`} type="button" aria-pressed={lens === definition.id} onclick={() => { lens = definition.id; }} data-testid={`credential-bindings-lens-${definition.id}`}><span>{definition.label}</span><span class="rounded border border-admin-border bg-admin-soft px-1.5 py-0.5 text-[11px] font-semibold text-admin-muted">{definition.count}</span></button>
      {/each}
    </div>
    <label class="mx-4 mb-3 flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border px-3 text-sm text-admin-muted lg:mb-0 lg:w-[340px]"><Search size={15} strokeWidth={1.8} /><span class="sr-only">Search credential bindings</span><input class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none placeholder:text-admin-muted" type="search" placeholder="Name, scope, origin..." bind:value={searchQuery} data-testid="credential-bindings-search" /></label>
  </div>
  <div class="max-h-[calc(100vh-360px)] min-h-64 overflow-auto">
    <table class="w-full min-w-[980px] border-collapse">
      <thead class="sticky top-0 z-10 bg-admin-soft"><tr class="border-b border-admin-border"><th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted">Binding</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Scope</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Injection</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Allowed origins</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Updated</th><th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted">Actions</th></tr></thead>
      <tbody>
        {#if visibleRows.length === 0}
          <tr><td class="px-4 py-14 text-center text-sm text-admin-muted" colspan="6" data-testid="credential-bindings-filter-empty">No credential bindings match the current filters.</td></tr>
        {:else}
          {#each visibleRows as row}
            <tr class="border-b border-admin-border last:border-b-0 hover:bg-admin-soft" data-testid="credential-bindings-list-row">
              <td class="w-[290px] px-4 py-3"><div class="grid min-w-0"><span class="truncate text-sm font-semibold text-admin-ink">{row.name}</span><span class="mt-1 truncate text-xs text-admin-muted">{row.provider} · {row.namespace}</span><span class="mt-1 truncate font-mono text-[11px] text-admin-muted">{row.id}</span></div></td>
              <td class="max-w-[180px] px-3 py-3"><span class={`inline-flex max-w-full rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.scopeTone)}`}><span class="truncate">{row.scope}</span></span></td>
              <td class="px-3 py-3 text-sm text-admin-ink">{row.injectionMode}</td>
              <td class="max-w-[260px] px-3 py-3 text-xs text-admin-muted"><span class="line-clamp-2">{row.origins}</span></td>
              <td class="px-3 py-3 text-xs text-admin-muted">{row.updatedAt}</td>
              <td class="px-4 py-3 text-right"><a class="inline-flex h-8 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft" href={`/admin-new/credential-bindings/${encodeURIComponent(row.id)}`} data-testid="credential-bindings-detail-link">Details</a></td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
</section>
