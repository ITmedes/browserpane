<script lang="ts">
  import { Ban, Pencil, Plus, Search, ShieldCheck } from '@lucide/svelte';
  import {
    buildServicePrincipalRows,
    servicePrincipalStateRequest,
  } from '$lib/identity/service-principal-view-model';
  import type {
    IdentityAccessReviewResponse,
    ServicePrincipalResource,
    UpsertServicePrincipalRequest,
  } from '$lib/identity/identity-types';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import ServicePrincipalEditor from './ServicePrincipalEditor.svelte';

  type ServicePrincipalCatalogProps = {
    readonly review: IdentityAccessReviewResponse;
    readonly busy?: boolean;
    readonly onCreate?: (request: UpsertServicePrincipalRequest) => boolean | Promise<boolean>;
    readonly onUpdate?: (id: string, request: UpsertServicePrincipalRequest) => boolean | Promise<boolean>;
  };

  let {
    review,
    busy = false,
    onCreate,
    onUpdate,
  }: ServicePrincipalCatalogProps = $props();
  let search = $state('');
  let selectedId = $state<string | null>(null);
  let editorMode = $state<'create' | 'edit' | null>(null);

  const rows = $derived(buildServicePrincipalRows(review.service_principals, review.projects, search));
  const selected = $derived(review.service_principals.find((entry) => entry.id === selectedId) ?? null);

  $effect(() => {
    if (!selectedId || !review.service_principals.some((entry) => entry.id === selectedId)) {
      selectedId = review.service_principals[0]?.id ?? null;
    }
  });

  async function save(request: UpsertServicePrincipalRequest): Promise<void> {
    const succeeded = editorMode === 'edit' && selected
      ? await onUpdate?.(selected.id, request)
      : await onCreate?.(request);
    if (succeeded) {
      editorMode = null;
    }
  }

  async function setState(state: 'active' | 'disabled'): Promise<void> {
    if (!selected || !onUpdate) {
      return;
    }
    await onUpdate(selected.id, servicePrincipalStateRequest(selected, state));
  }

  function select(id: string): void {
    selectedId = id;
    editorMode = null;
  }
</script>

<div class="grid min-w-0 gap-4" data-testid="service-principal-catalog">
  <section class="min-w-0 rounded-md border border-admin-border bg-admin-panel">
    <div class="flex flex-col gap-3 border-b border-admin-border px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
      <div class="min-w-0">
        <h2 class="m-0 text-sm font-semibold text-admin-ink">Service-principal registry</h2>
        <p class="m-0 mt-1 text-xs text-admin-muted">External automation identity metadata and current delegation correlation.</p>
      </div>
      <button class="inline-flex h-9 w-fit items-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:opacity-60" type="button" onclick={() => { editorMode = 'create'; }} disabled={busy || !onCreate} data-testid="service-principal-create">
        <Plus size={15} strokeWidth={1.9} /><span>Register principal</span>
      </button>
    </div>

    <div class="flex flex-col gap-3 border-b border-admin-border bg-admin-soft/50 px-4 py-3 md:flex-row md:items-center md:justify-between">
      <label class="flex h-9 min-w-0 items-center gap-2 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-muted md:w-[380px]">
        <Search size={15} strokeWidth={1.8} aria-hidden="true" />
        <span class="sr-only">Search service principals</span>
        <input class="min-w-0 flex-1 border-0 bg-transparent text-sm text-admin-ink outline-none" type="search" placeholder="Name, client id, issuer, project, scope..." bind:value={search} data-testid="service-principal-search" />
      </label>
      <span class="text-xs text-admin-muted" data-testid="service-principal-count">{rows.length} of {review.service_principals.length}</span>
    </div>

    <div class="max-h-[420px] min-h-44 overflow-auto">
      <table class="w-full min-w-[920px] border-collapse">
        <thead class="sticky top-0 z-10 bg-admin-soft"><tr class="border-b border-admin-border"><th class="px-4 py-2 text-left text-xs font-bold uppercase text-admin-muted">Principal</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">State</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Projects</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Intended scopes</th><th class="px-3 py-2 text-left text-xs font-bold uppercase text-admin-muted">Delegation</th><th class="px-4 py-2 text-right text-xs font-bold uppercase text-admin-muted">Select</th></tr></thead>
        <tbody>
          {#if rows.length === 0}
            <tr><td class="px-4 py-12 text-center text-sm text-admin-muted" colspan="6" data-testid="service-principal-empty">No service principals match the current filter.</td></tr>
          {:else}
            {#each rows as row}
              <tr class={`border-b border-admin-border last:border-b-0 ${selectedId === row.id ? 'bg-blue-50/60' : 'hover:bg-admin-soft'}`} data-testid="service-principal-row">
                <td class="px-4 py-3"><p class="m-0 text-sm font-semibold text-admin-ink">{row.name}</p><p class="m-0 mt-1 font-mono text-xs text-admin-muted">{row.clientId}</p><p class="m-0 mt-1 max-w-xs truncate text-[11px] text-admin-muted">{row.issuer}</p></td>
                <td class="px-3 py-3"><span class={`rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(row.stateTone)}`}>{row.state}</span></td>
                <td class="max-w-48 px-3 py-3 text-xs text-admin-muted">{row.projects}</td>
                <td class="max-w-48 px-3 py-3 text-xs text-admin-muted">{row.scopes}</td>
                <td class="px-3 py-3 text-xs text-admin-muted">{row.delegation}</td>
                <td class="px-4 py-3 text-right"><button class="h-8 rounded-md border border-admin-border bg-admin-panel px-3 text-xs font-semibold text-admin-ink hover:bg-admin-soft" type="button" onclick={() => select(row.id)} aria-pressed={selectedId === row.id} data-testid="service-principal-select">{selectedId === row.id ? 'Selected' : 'Select'}</button></td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
  </section>

  <section class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid="service-principal-selected">
    {#if selected}
      <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0"><p class="m-0 text-xs font-semibold uppercase text-admin-muted">Selected principal</p><h3 class="m-0 mt-1 text-lg font-semibold text-admin-ink">{selected.name}</h3><p class="m-0 mt-1 break-all font-mono text-xs text-admin-muted">{selected.client_id}</p></div>
        <div class="flex flex-wrap gap-2">
          <button class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:opacity-60" type="button" onclick={() => { editorMode = 'edit'; }} disabled={busy || !onUpdate} data-testid="service-principal-edit"><Pencil size={15} /><span>Edit</span></button>
          {#if selected.state === 'active'}
            <button class="inline-flex h-9 items-center gap-2 rounded-md border border-red-200 bg-red-50 px-3 text-sm font-semibold text-red-700 disabled:opacity-60" type="button" onclick={() => void setState('disabled')} disabled={busy || !onUpdate} data-testid="service-principal-disable"><Ban size={15} /><span>Disable</span></button>
          {:else}
            <button class="inline-flex h-9 items-center gap-2 rounded-md border border-emerald-200 bg-emerald-50 px-3 text-sm font-semibold text-emerald-700 disabled:opacity-60" type="button" onclick={() => void setState('active')} disabled={busy || !onUpdate} data-testid="service-principal-enable"><ShieldCheck size={15} /><span>Re-enable</span></button>
          {/if}
        </div>
      </div>
      <dl class="m-0 mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4"><div><dt class="text-xs font-semibold text-admin-muted">Issuer</dt><dd class="m-0 mt-1 break-all text-xs text-admin-ink">{selected.issuer}</dd></div><div><dt class="text-xs font-semibold text-admin-muted">Delegated sessions</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{selected.active_delegated_session_count} / {selected.delegated_session_count} active</dd></div><div><dt class="text-xs font-semibold text-admin-muted">Last seen</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{selected.last_seen_at ?? 'Not observed'}</dd></div><div><dt class="text-xs font-semibold text-admin-muted">Updated</dt><dd class="m-0 mt-1 text-sm text-admin-ink">{selected.updated_at}</dd></div></dl>
    {:else}
      <AdminMessage tone="info" title="No service principal selected" message="Register a principal or clear the current filter." />
    {/if}
  </section>

  {#if editorMode}
    <section class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid="service-principal-editor-shell">
      {#key `${editorMode}:${selected?.id ?? 'new'}`}
        <ServicePrincipalEditor principal={review.principal} projects={review.projects} resource={editorMode === 'edit' ? selected : null} disabled={busy} onSave={save} onCancel={() => { editorMode = null; }} />
      {/key}
    </section>
  {/if}
</div>
