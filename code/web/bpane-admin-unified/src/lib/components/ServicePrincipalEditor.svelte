<script lang="ts">
  import type { IdentityPrincipalResource, IdentityServicePrincipalReviewResource, UpsertServicePrincipalRequest } from '$lib/identity/identity-types';
  import {
    createServicePrincipalDraft,
    editServicePrincipalDraft,
    validateServicePrincipalDraft,
    type ServicePrincipalDraft,
  } from '$lib/identity/service-principal-view-model';
  import type { ProjectResource } from '$lib/projects/project-types';
  import FieldFeedback from './FieldFeedback.svelte';

  type ServicePrincipalEditorProps = {
    readonly principal: IdentityPrincipalResource;
    readonly projects: readonly ProjectResource[];
    readonly resource?: IdentityServicePrincipalReviewResource | null;
    readonly disabled?: boolean;
    readonly onSave?: (request: UpsertServicePrincipalRequest) => void | Promise<void>;
    readonly onCancel?: () => void;
  };

  let {
    principal,
    projects,
    resource = null,
    disabled = false,
    onSave,
    onCancel,
  }: ServicePrincipalEditorProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<ServicePrincipalDraft>(resource
    ? editServicePrincipalDraft(resource)
    : createServicePrincipalDraft(principal));
  const validation = $derived(validateServicePrincipalDraft(draft));

  function toggleProject(projectId: string, checked: boolean): void {
    draft = {
      ...draft,
      allowedProjectIds: checked
        ? [...new Set([...draft.allowedProjectIds, projectId])]
        : draft.allowedProjectIds.filter((id) => id !== projectId),
    };
  }

  function save(): void {
    if (validation.request) {
      void onSave?.(validation.request);
    }
  }
</script>

<form class="grid min-w-0 gap-4" onsubmit={(event) => { event.preventDefault(); save(); }} data-testid="service-principal-editor">
  <div class="border-b border-admin-border pb-3">
    <h3 class="m-0 text-base font-semibold text-admin-ink">{resource ? 'Edit service principal' : 'Register service principal'}</h3>
    <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">Register external OIDC metadata only. Client secrets and raw tokens never belong in this form.</p>
  </div>

  <div class="grid items-start gap-4 md:grid-cols-2">
    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Name</span>
      <input class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.name} disabled={disabled} data-testid="service-principal-name" />
      <FieldFeedback errors={validation.fieldErrors.name} testId="service-principal-name-error" />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">State</span>
      <select class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.state} disabled={disabled} data-testid="service-principal-state">
        <option value="active">active</option>
        <option value="disabled">disabled</option>
      </select>
      <FieldFeedback hint="Disabled registrations cannot be assigned as new automation delegates." />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">External client id</span>
      <input class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.clientId} disabled={disabled} autocomplete="off" data-testid="service-principal-client-id" />
      <FieldFeedback errors={validation.fieldErrors.clientId} testId="service-principal-client-id-error" />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Issuer</span>
      <input class="h-10 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.issuer} disabled={disabled} autocomplete="off" data-testid="service-principal-issuer" />
      <FieldFeedback errors={validation.fieldErrors.issuer} hint="Exact external OIDC issuer identifier." testId="service-principal-issuer-error" />
    </label>

    <label class="grid min-w-0 content-start gap-1.5 text-sm md:col-span-2">
      <span class="font-medium text-admin-ink">Description</span>
      <textarea class="min-h-20 rounded-md border border-admin-border bg-white px-3 py-2 text-sm text-admin-ink outline-none focus:border-admin-accent" bind:value={draft.description} disabled={disabled} data-testid="service-principal-description"></textarea>
      <FieldFeedback hint="Operator-facing purpose of this external automation identity." />
    </label>
  </div>

  <fieldset class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-3" disabled={disabled}>
    <legend class="px-1 text-sm font-semibold text-admin-ink">Allowed project metadata</legend>
    <p class="m-0 mb-3 text-xs leading-5 text-admin-muted">Intended project scope for review and future authorization. It is not complete grant enforcement yet.</p>
    {#if projects.length === 0}
      <p class="m-0 text-sm text-admin-muted">No projects are available.</p>
    {:else}
      <div class="grid gap-2 sm:grid-cols-2">
        {#each projects as project}
          <label class="flex items-start gap-2 rounded-md border border-admin-border bg-admin-panel px-3 py-2 text-sm">
            <input class="mt-0.5 h-4 w-4 rounded border-admin-border text-admin-accent" type="checkbox" checked={draft.allowedProjectIds.includes(project.id)} onchange={(event) => toggleProject(project.id, (event.currentTarget as HTMLInputElement).checked)} data-testid="service-principal-project-option" data-project-id={project.id} />
            <span class="min-w-0"><span class="block font-medium text-admin-ink">{project.name}</span><span class="block truncate font-mono text-[11px] text-admin-muted">{project.id}</span></span>
          </label>
        {/each}
      </div>
    {/if}
  </fieldset>

  <div class="grid items-start gap-4 md:grid-cols-2">
    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Intended scopes</span>
      <textarea class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" placeholder="session:delegate&#10;session:read" bind:value={draft.scopesText} disabled={disabled} data-testid="service-principal-scopes"></textarea>
      <FieldFeedback hint="One scope per line. These are review metadata until grant enforcement is implemented." />
    </label>
    <label class="grid min-w-0 content-start gap-1.5 text-sm">
      <span class="font-medium text-admin-ink">Labels</span>
      <textarea class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent" placeholder="team=platform" bind:value={draft.labelsText} disabled={disabled} data-testid="service-principal-labels"></textarea>
      <FieldFeedback errors={validation.fieldErrors.labels} hint="One key=value label per line." testId="service-principal-labels-error" />
    </label>
  </div>

  <div class="flex flex-wrap justify-end gap-2 border-t border-admin-border pt-4">
    <button class="inline-flex h-10 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft" type="button" onclick={onCancel} disabled={disabled} data-testid="service-principal-cancel">Cancel</button>
    <button class="inline-flex h-10 items-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white disabled:cursor-not-allowed disabled:opacity-60" type="submit" disabled={disabled || !validation.valid} data-testid="service-principal-save">{resource ? 'Save service principal' : 'Register service principal'}</button>
  </div>
</form>
