<script lang="ts">
  import {
    createProjectEditDraft,
    hasProjectEditChanges,
    validateProjectEdit,
    type ProjectEditDraft,
  } from '$lib/projects/project-edit-view-model';
  import type { ProjectPolicyOptionsLoadState } from '$lib/projects/project-detail-state';
  import type { ProjectResource, UpsertProjectRequest } from '$lib/projects/project-types';
  import ProjectPolicyEditor from './ProjectPolicyEditor.svelte';
  import ProjectQuotaEditor from './ProjectQuotaEditor.svelte';

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

<section class="rounded-md border border-admin-border bg-admin-panel p-4" data-testid="project-edit-form">
  <div class="flex flex-col gap-1 border-b border-admin-border pb-3">
    <h3 class="m-0 text-sm font-semibold text-admin-ink">Edit project</h3>
    <p class="m-0 text-xs text-admin-muted">Update metadata, project policy, resource allow-lists, and quotas.</p>
  </div>

  <div class="mt-4 grid gap-4">
    <label class="grid gap-1 text-sm">
      <span class="font-medium text-admin-ink">Name</span>
      <input
        class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
        type="text"
        bind:value={draft.name}
        disabled={disabled}
        data-testid="project-edit-name"
      />
    </label>

    <label class="grid gap-1 text-sm">
      <span class="font-medium text-admin-ink">Description</span>
      <textarea
        class="min-h-20 rounded-md border border-admin-border bg-white px-3 py-2 text-sm text-admin-ink outline-none focus:border-admin-accent"
        bind:value={draft.description}
        disabled={disabled}
        data-testid="project-edit-description"
      ></textarea>
    </label>

    <label class="grid gap-1 text-sm">
      <span class="font-medium text-admin-ink">State</span>
      <select
        class="h-10 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none focus:border-admin-accent"
        bind:value={draft.state}
        disabled={disabled}
        data-testid="project-edit-state"
      >
        <option value="active">active</option>
        <option value="archived">archived</option>
      </select>
    </label>

    <label class="grid gap-1 text-sm">
      <span class="font-medium text-admin-ink">Labels</span>
      <textarea
        class="min-h-24 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs text-admin-ink outline-none focus:border-admin-accent"
        placeholder="team=support&#10;env=prod"
        bind:value={draft.labelsText}
        disabled={disabled}
        data-testid="project-edit-labels"
      ></textarea>
      <span class="text-xs text-admin-muted">One `key=value` label per line.</span>
    </label>

    <ProjectPolicyEditor
      {draft}
      {disabled}
      {policyOptionsState}
      onDraftChange={updateDraft}
    />

    <ProjectQuotaEditor
      {draft}
      {disabled}
      onDraftChange={updateDraft}
    />
  </div>

  {#if validation.errors.length > 0}
    <div class="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-800" role="alert" data-testid="project-edit-validation">
      {#each validation.errors as error}
        <p class="m-0">{error}</p>
      {/each}
    </div>
  {/if}

  <div class="mt-4 flex flex-wrap justify-end gap-2">
    <button
      class="inline-flex h-9 items-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={reset}
      disabled={disabled || !changed}
      data-testid="project-edit-cancel"
    >
      Cancel
    </button>
    <button
      class="inline-flex h-9 items-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-medium text-white hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={save}
      disabled={disabled || !changed || !validation.valid}
      data-testid="project-edit-save"
    >
      Save changes
    </button>
  </div>
</section>
