<script lang="ts">
  import {
    createIdentityMappingDraft,
    editIdentityMappingDraft,
    selectMappingServicePrincipal,
    validateIdentityMappingDraft,
    type IdentityMappingDraft,
  } from '$lib/identity/identity-mapping-view-model';
  import type {
    IdentityMappingKind,
    IdentityMappingReviewResource,
    IdentityPrincipalResource,
    IdentityServicePrincipalReviewResource,
    UpsertIdentityMappingRequest,
  } from '$lib/identity/identity-types';
  import type { ProjectResource } from '$lib/projects/project-types';
  import FieldFeedback from './FieldFeedback.svelte';

  type IdentityMappingEditorProps = {
    readonly principal: IdentityPrincipalResource;
    readonly projects: readonly ProjectResource[];
    readonly servicePrincipals: readonly IdentityServicePrincipalReviewResource[];
    readonly resource?: IdentityMappingReviewResource | null;
    readonly disabled?: boolean;
    readonly onSave?: (request: UpsertIdentityMappingRequest) => void | Promise<void>;
    readonly onCancel?: () => void;
  };

  let {
    principal,
    projects,
    servicePrincipals,
    resource = null,
    disabled = false,
    onSave,
    onCancel,
  }: IdentityMappingEditorProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<IdentityMappingDraft>(resource
    ? editIdentityMappingDraft(resource)
    : createIdentityMappingDraft(principal, projects));
  const validation = $derived(validateIdentityMappingDraft(draft));

  function changeKind(event: Event): void {
    const kind = (event.currentTarget as HTMLSelectElement).value as IdentityMappingKind;
    if (kind === 'service_principal') {
      draft = selectMappingServicePrincipal({ ...draft, kind, claimName: '' }, servicePrincipals[0] ?? null);
      return;
    }
    draft = {
      ...draft,
      kind,
      claimName: kind === 'claim' ? draft.claimName : '',
      servicePrincipalId: '',
    };
  }

  function changeServicePrincipal(event: Event): void {
    const id = (event.currentTarget as HTMLSelectElement).value;
    draft = selectMappingServicePrincipal(
      draft,
      servicePrincipals.find((entry) => entry.id === id) ?? null,
    );
  }

  function save(): void {
    if (validation.request) {
      void onSave?.(validation.request);
    }
  }
</script>

<form class="grid min-w-0 gap-4" onsubmit={(event) => { event.preventDefault(); save(); }} data-testid="identity-mapping-editor">
  <div class="border-b border-admin-border pb-3">
    <h3 class="m-0 text-base font-semibold text-admin-ink">{resource ? 'Edit identity mapping' : 'Create identity mapping'}</h3>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">Map a sanitized external identity signal to project review metadata without storing raw tokens or unrestricted claims.</p>
  </div>

  <div class="grid items-start gap-4 md:grid-cols-2">
    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Name</span>
      <input class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.name} disabled={disabled} data-testid="identity-mapping-name" />
      <FieldFeedback errors={validation.fieldErrors.name} testId="identity-mapping-name-error" />
    </label>
    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">State</span>
      <select class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.state} disabled={disabled} data-testid="identity-mapping-state"><option value="active">active</option><option value="disabled">disabled</option></select>
      <FieldFeedback hint="Disabled mappings remain visible but are ineffective in access review." />
    </label>
    <label class="grid min-w-0 content-start gap-1.5 text-sm md:col-span-2">
      <span class="font-medium text-admin-ink">Description</span>
      <textarea class="min-h-20 rounded-md border border-admin-border bg-white px-3 py-2 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.description} disabled={disabled} data-testid="identity-mapping-description"></textarea>
      <FieldFeedback hint="Optional operator-facing purpose for this mapping." />
    </label>
  </div>

  <section class="rounded-md border border-admin-border bg-admin-soft/50 p-3">
    <div class="grid items-start gap-4 md:grid-cols-2">
      <label class="grid min-w-0 content-start gap-1.5 text-sm">
        <span class="font-medium text-admin-ink">Signal kind</span>
        <select class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" value={draft.kind} onchange={changeKind} disabled={disabled} data-testid="identity-mapping-kind">
          <option value="user">user</option><option value="group">group</option><option value="claim">allowlisted claim</option><option value="service_principal">registered service principal</option>
        </select>
        <FieldFeedback hint="Only safe, explicitly represented identity signals are stored." />
      </label>
      <label class="grid min-w-0 content-start gap-1.5 text-sm">
        <span class="font-medium text-admin-ink">Project</span>
        <select class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.projectId} disabled={disabled} data-testid="identity-mapping-project-id">
          <option value="">Select a project...</option>
          {#each projects as project}<option value={project.id}>{project.name} · {project.state}</option>{/each}
        </select>
        <FieldFeedback errors={validation.fieldErrors.projectId} hint="Project names are resolved from the current access review." testId="identity-mapping-project-id-error" />
      </label>

      {#if draft.kind === 'service_principal'}
        <label class="grid min-w-0 content-start gap-1.5 text-sm md:col-span-2">
          <span class="font-medium text-admin-ink">Registered service principal</span>
          <select class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" value={draft.servicePrincipalId} onchange={changeServicePrincipal} disabled={disabled} data-testid="identity-mapping-service-principal-id">
            <option value="">Select a service principal...</option>
            {#each servicePrincipals as servicePrincipal}<option value={servicePrincipal.id}>{servicePrincipal.name} · {servicePrincipal.client_id} · {servicePrincipal.state}</option>{/each}
          </select>
          <FieldFeedback errors={validation.fieldErrors.servicePrincipalId} testId="identity-mapping-service-principal-id-error" />
        </label>
      {/if}

      <label class="grid min-w-0 content-start gap-1.5 text-sm">
        <span class="font-medium text-admin-ink">Issuer</span>
        <input class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent disabled:bg-admin-soft" bind:value={draft.issuer} disabled={disabled || draft.kind === 'service_principal'} data-testid="identity-mapping-issuer" />
        <FieldFeedback errors={validation.fieldErrors.issuer} testId="identity-mapping-issuer-error" />
      </label>
      <label class="grid min-w-0 content-start gap-1.5 text-sm">
        <span class="font-medium text-admin-ink">External identity</span>
        <input class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent disabled:bg-admin-soft" bind:value={draft.externalId} disabled={disabled || draft.kind === 'service_principal'} data-testid="identity-mapping-external-id" />
        <FieldFeedback errors={validation.fieldErrors.externalId} testId="identity-mapping-external-id-error" />
      </label>
      {#if draft.kind === 'claim'}
        <label class="grid min-w-0 content-start gap-1.5 text-sm md:col-span-2">
          <span class="font-medium text-admin-ink">Allowlisted claim name</span>
          <input class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.claimName} disabled={disabled} data-testid="identity-mapping-claim-name" />
          <FieldFeedback errors={validation.fieldErrors.claimName} hint="Only configure a claim explicitly allowlisted by the identity normalization boundary." testId="identity-mapping-claim-name-error" />
        </label>
      {/if}
    </div>
  </section>

  <div class="grid items-start gap-4 md:grid-cols-2">
    <label class="grid min-w-0 content-start gap-1.5 text-sm"><span class="font-medium text-admin-ink">Intended scopes</span><textarea class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.scopesText} disabled={disabled} data-testid="identity-mapping-scopes"></textarea><FieldFeedback hint="Review metadata until generalized grant enforcement is implemented." /></label>
    <label class="grid min-w-0 content-start gap-1.5 text-sm"><span class="font-medium text-admin-ink">Labels</span><textarea class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" placeholder="source=keycloak" bind:value={draft.labelsText} disabled={disabled} data-testid="identity-mapping-labels"></textarea><FieldFeedback errors={validation.fieldErrors.labels} hint="One key=value label per line." testId="identity-mapping-labels-error" /></label>
  </div>

  <div class="flex flex-wrap justify-end gap-2 border-t border-admin-border pt-4">
    <button class="inline-flex h-10 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft" type="button" onclick={onCancel} disabled={disabled} data-testid="identity-mapping-cancel">Cancel</button>
    <button class="inline-flex h-10 items-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-60" type="submit" disabled={disabled || !validation.valid} data-testid="identity-mapping-save">{resource ? 'Save identity mapping' : 'Create identity mapping'}</button>
  </div>
</form>
