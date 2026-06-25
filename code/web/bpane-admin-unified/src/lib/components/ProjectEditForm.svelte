<script lang="ts">
  import {
    createProjectEditDraft,
    hasProjectEditChanges,
    validateProjectEdit,
    type ProjectEditDraft,
  } from '$lib/projects/project-edit-view-model';
  import { buildProjectInspectorModel } from '$lib/projects/project-inspector-view-model';
  import type { ProjectPolicyOptionsLoadState } from '$lib/projects/project-detail-state';
  import type { ProjectResource, UpsertProjectRequest } from '$lib/projects/project-types';
  import AdminMessage from './AdminMessage.svelte';
  import ProjectPolicyEditor from './ProjectPolicyEditor.svelte';
  import ProjectQuotaEditor from './ProjectQuotaEditor.svelte';
  import ProjectStatusSummary from './ProjectStatusSummary.svelte';

  type ProjectEditFormProps = {
    readonly project: ProjectResource;
    readonly disabled?: boolean;
    readonly policyOptionsState?: ProjectPolicyOptionsLoadState;
    readonly onSave?: (request: UpsertProjectRequest) => void | Promise<void>;
  };

  let {
    project,
    disabled = false,
    policyOptionsState = { status: 'idle' },
    onSave,
  }: ProjectEditFormProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<ProjectEditDraft>(createProjectEditDraft(project));

  const validation = $derived(validateProjectEdit(project, draft));
  const changed = $derived(hasProjectEditChanges(project, draft));
  const statusModel = $derived(buildProjectInspectorModel(project));
  const changeStatusLabel = $derived(disabled ? 'Read-only access' : changed ? 'Unsaved changes' : 'No changes');
  const changeStatusClass = $derived(disabled
    ? 'border-admin-border bg-admin-soft text-admin-muted'
    : changed
      ? 'border-amber-200 bg-amber-50 text-amber-900'
      : 'border-emerald-200 bg-emerald-50 text-emerald-800');

  function reset(): void {
    draft = createProjectEditDraft(project);
  }

  function updateDraft(nextDraft: ProjectEditDraft): void {
    draft = nextDraft;
  }

  function save(): void {
    if (!validation.request) {
      return;
    }
    void onSave?.(validation.request);
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5" data-testid="project-edit-form">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-base font-semibold text-admin-ink">Project settings</h3>
      <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
        Review current status and update metadata, policy, resource allow-lists, and quotas.
      </p>
    </div>
    <span class={`inline-flex w-fit shrink-0 rounded-full border px-2.5 py-1 text-xs font-semibold ${changeStatusClass}`}>
      {changeStatusLabel}
    </span>
  </div>

  <div class="mt-5 grid gap-4">
    <ProjectStatusSummary model={statusModel} />

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="project-metadata-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Metadata</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Keep the human-readable project identity and lifecycle state aligned with operations.
        </p>
      </div>

      <div class="mt-4 grid gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
        <label class="grid gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Name</span>
          <input
            class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.name}
            disabled={disabled}
            autocomplete="off"
            data-testid="project-edit-name"
          />
          {#if validation.fieldErrors.name?.length}
            <AdminMessage
              tone="error"
              density="compact"
              items={validation.fieldErrors.name}
              testId="project-edit-name-error"
            />
          {/if}
        </label>

        <label class="grid gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">State</span>
          <select
            class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.state}
            disabled={disabled}
            data-testid="project-edit-state"
          >
            <option value="active">active</option>
            <option value="archived">archived</option>
          </select>
        </label>

        <label class="grid gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Description</span>
          <textarea
            class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 text-sm leading-6 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.description}
            disabled={disabled}
            data-testid="project-edit-description"
          ></textarea>
        </label>

        <label class="grid gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Labels</span>
          <textarea
            class="min-h-28 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            placeholder="team=support&#10;env=prod"
            bind:value={draft.labelsText}
            disabled={disabled}
            spellcheck="false"
            data-testid="project-edit-labels"
          ></textarea>
          <span class="text-xs leading-5 text-admin-muted">One `key=value` label per line.</span>
          {#if validation.fieldErrors.labels?.length}
            <AdminMessage
              tone="error"
              density="compact"
              items={validation.fieldErrors.labels}
              testId="project-edit-labels-error"
            />
          {/if}
        </label>
      </div>
    </section>

    <ProjectPolicyEditor
      {draft}
      {disabled}
      {policyOptionsState}
      fieldErrors={validation.fieldErrors}
      onDraftChange={updateDraft}
    />

    <ProjectQuotaEditor
      {draft}
      {disabled}
      fieldErrors={validation.fieldErrors}
      onDraftChange={updateDraft}
    />
  </div>

  <div class="sticky bottom-0 z-10 -mx-4 -mb-4 mt-5 flex flex-col gap-2 border-t border-admin-border bg-admin-panel/95 px-4 py-3 backdrop-blur sm:-mx-5 sm:-mb-5 sm:flex-row sm:items-center sm:justify-end sm:px-5">
    <button
      class="inline-flex h-10 items-center justify-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={reset}
      disabled={disabled || !changed}
      data-testid="project-edit-cancel"
    >
      Cancel
    </button>
    <button
      class="inline-flex h-10 items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={save}
      disabled={disabled || !changed || !validation.valid}
      data-testid="project-edit-save"
    >
      Save changes
    </button>
  </div>
</section>
