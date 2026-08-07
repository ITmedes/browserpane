<script lang="ts">
  import { Ban, Pencil, Plus, Search, ShieldCheck } from '@lucide/svelte';
  import {
    buildIdentityMappingRows,
    identityMappingStateRequest,
  } from '$lib/identity/identity-mapping-view-model';
  import type {
    IdentityAccessReviewResponse,
    UpsertIdentityMappingRequest,
  } from '$lib/identity/identity-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import IdentityMappingEditor from './IdentityMappingEditor.svelte';

  type IdentityMappingCatalogProps = {
    readonly review: IdentityAccessReviewResponse;
    readonly busy?: boolean;
    readonly onCreate?: (request: UpsertIdentityMappingRequest) => boolean | Promise<boolean>;
    readonly onUpdate?: (id: string, request: UpsertIdentityMappingRequest) => boolean | Promise<boolean>;
  };

  let { review, busy = false, onCreate, onUpdate }: IdentityMappingCatalogProps = $props();
  let search = $state('');
  let selectedId = $state<string | null>(null);
  let editorMode = $state<'create' | 'edit' | null>(null);
  const rows = $derived(buildIdentityMappingRows(review.identity_mappings, review.projects, search));
  const selected = $derived(review.identity_mappings.find((entry) => entry.id === selectedId) ?? null);

  $effect(() => {
    if (!selectedId || !review.identity_mappings.some((entry) => entry.id === selectedId)) {
      selectedId = review.identity_mappings[0]?.id ?? null;
    }
  });

  async function save(request: UpsertIdentityMappingRequest): Promise<void> {
    const succeeded = editorMode === 'edit' && selected
      ? await onUpdate?.(selected.id, request)
      : await onCreate?.(request);
    if (succeeded) {
      editorMode = null;
    }
  }

  async function setState(state: 'active' | 'disabled'): Promise<void> {
    if (selected && onUpdate) {
      await onUpdate(selected.id, identityMappingStateRequest(selected, state));
    }
  }

  function select(id: string): void {
    selectedId = id;
    editorMode = null;
  }
</script>

<div class="grid min-w-0 gap-4" data-testid="identity-mapping-catalog">
  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel">
    <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between"><div class="min-w-0"><h2 class="m-0 text-sm font-semibold text-admin-ink">Identity mappings</h2><p class="m-0 mt-1 text-xs text-admin-muted">Sanitized external identity signals associated with project review metadata.</p></div><button class="inline-flex h-9 w-fit items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60" type="button" onclick={() => { editorMode = 'create'; }} disabled={busy || !onCreate} data-testid="identity-mapping-create"><Plus size={15} /><span>Create mapping</span></button></div>
    <div class="flex flex-col gap-3 border-b border-admin-border bg-admin-soft/50 px-4 py-3 md:flex-row md:items-center md:justify-between"><label class="flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-muted md:w-[380px]"><Search size={15} /><span class="sr-only">Search identity mappings</span><input class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none" type="search" placeholder="Name, signal, issuer, project, scope..." bind:value={search} data-testid="identity-mapping-search" /></label><span class="text-xs text-admin-muted" data-testid="identity-mapping-count">{rows.length} of {review.identity_mappings.length}</span></div>
    <div class="max-h-[420px] min-h-44 overflow-auto"><table class="w-full min-w-[980px] border-collapse"><thead class="sticky top-0 z-10 bg-admin-soft"><tr class="border-b border-admin-border"><th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted">Mapping</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">State</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Effect</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">External identity</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Project</th><th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted">Select</th></tr></thead><tbody>
      {#if rows.length === 0}<tr><td class="px-4 py-12 text-center text-sm text-admin-muted" colspan="6" data-testid="identity-mapping-empty">No identity mappings match the current filter.</td></tr>{:else}
        {#each rows as row}<tr class={`border-b border-admin-border last:border-b-0 ${selectedId === row.id ? 'bg-blue-50/60' : 'hover:bg-admin-soft'}`} data-testid="identity-mapping-row"><td class="px-4 py-3"><p class="m-0 text-sm font-semibold text-admin-ink">{row.name}</p><p class="m-0 mt-1 text-xs text-admin-muted">{row.kind}</p><p class="m-0 mt-1 font-mono text-[11px] text-admin-muted">{row.id}</p></td><td class="px-3 py-3"><span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>{row.state}</span></td><td class="px-3 py-3"><span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.effectivenessTone)}`}>{row.effectiveness}</span></td><td class="max-w-64 px-3 py-3"><p class="m-0 break-all font-mono text-xs text-admin-ink">{row.externalIdentity}</p><p class="m-0 mt-1 max-w-64 truncate text-[11px] text-admin-muted">{row.issuer}</p></td><td class="px-3 py-3 text-xs text-admin-muted">{row.project}</td><td class="px-4 py-3 text-right"><button class="h-8 rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft" type="button" onclick={() => select(row.id)} aria-pressed={selectedId === row.id} data-testid="identity-mapping-select">{selectedId === row.id ? 'Selected' : 'Select'}</button></td></tr>{/each}
      {/if}
    </tbody></table></div>
  </section>

  <section class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid="identity-mapping-selected">
    {#if selected}
      <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between"><div class="min-w-0"><p class="m-0 text-xs font-semibold uppercase text-admin-muted">Selected mapping</p><h3 class="m-0 mt-1 text-lg font-semibold text-admin-ink">{selected.name}</h3><p class="m-0 mt-1 break-all font-mono text-xs text-admin-muted">{selected.claim_name ? `${selected.claim_name}=${selected.external_id}` : selected.external_id}</p></div><div class="flex flex-wrap gap-2"><button class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:opacity-60" type="button" onclick={() => { editorMode = 'edit'; }} disabled={busy || !onUpdate} data-testid="identity-mapping-edit"><Pencil size={15} /><span>Edit</span></button>{#if selected.state === 'active'}<button class="inline-flex h-9 items-center gap-2 rounded-md border border-red-200 bg-red-50 px-3 text-sm font-semibold text-red-700 disabled:opacity-60" type="button" onclick={() => void setState('disabled')} disabled={busy || !onUpdate} data-testid="identity-mapping-disable"><Ban size={15} /><span>Disable</span></button>{:else}<button class="inline-flex h-9 items-center gap-2 rounded-md border border-emerald-200 bg-emerald-50 px-3 text-sm font-semibold text-emerald-700 disabled:opacity-60" type="button" onclick={() => void setState('active')} disabled={busy || !onUpdate} data-testid="identity-mapping-enable"><ShieldCheck size={15} /><span>Re-enable</span></button>{/if}</div></div>
      <dl class="m-0 mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4"><div><dt class="text-xs font-semibold text-admin-muted">Issuer</dt><dd class="m-0 mt-1 break-all text-xs text-admin-ink">{selected.issuer}</dd></div><div><dt class="text-xs font-semibold text-admin-muted">Project id</dt><dd class="m-0 mt-1 break-all font-mono text-xs text-admin-ink">{selected.project_id}</dd></div><div><dt class="text-xs font-semibold text-admin-muted">Current effect</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{selected.effective_for_principal ? 'Effective for current principal' : 'Not effective for current principal'}</dd></div><div><dt class="text-xs font-semibold text-admin-muted">Updated</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{selected.updated_at}</dd></div></dl>
    {:else}<AdminMessage tone="info" title="No identity mapping selected" message="Create a mapping or clear the current filter." />{/if}
  </section>

  {#if editorMode}<section class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid="identity-mapping-editor-shell">{#key `${editorMode}:${selected?.id ?? 'new'}`}<IdentityMappingEditor principal={review.principal} projects={review.projects} servicePrincipals={review.service_principals} resource={editorMode === 'edit' ? selected : null} disabled={busy} onSave={save} onCancel={() => { editorMode = null; }} />{/key}</section>{/if}
</div>
