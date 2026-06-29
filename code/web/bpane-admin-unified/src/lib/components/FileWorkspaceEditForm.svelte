<script lang="ts">
  import {
    createNewFileWorkspaceEditDraft,
    hasNewFileWorkspaceEditChanges,
    mergeProjectsWithSelected,
    validateFileWorkspaceEdit,
    type FileWorkspaceEditDraft,
  } from '$lib/file-workspaces/file-workspace-edit-view-model';
  import type { FileWorkspaceProjectOptionsLoadState } from '$lib/file-workspaces/file-workspace-detail-state';
  import type { CreateFileWorkspaceRequest } from '$lib/file-workspaces/file-workspace-types';
  import AdminMessage from './AdminMessage.svelte';
  import FieldFeedback from './FieldFeedback.svelte';

  type FileWorkspaceEditFormProps = {
    readonly disabled?: boolean;
    readonly projectOptionsState?: FileWorkspaceProjectOptionsLoadState;
    readonly onSave?: (request: CreateFileWorkspaceRequest) => void | Promise<void>;
  };

  let {
    disabled = false,
    projectOptionsState = { status: 'idle' },
    onSave,
  }: FileWorkspaceEditFormProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<FileWorkspaceEditDraft>(createNewFileWorkspaceEditDraft());

  const validation = $derived(validateFileWorkspaceEdit(draft));
  const changed = $derived(hasNewFileWorkspaceEditChanges(draft));
  const projectOptions = $derived(projectOptionsState.status === 'ready'
    ? mergeProjectsWithSelected(projectOptionsState.projects, draft.projectId)
    : mergeProjectsWithSelected([], draft.projectId));
  const changeStatusLabel = $derived(disabled
    ? 'Read-only access'
    : validation.valid
      ? 'Ready to create'
      : 'Workspace draft');
  const changeStatusClass = $derived(disabled
    ? 'border-admin-border bg-admin-soft text-admin-muted'
    : validation.valid
      ? 'border-emerald-200 bg-emerald-50 text-emerald-800'
      : 'border-amber-200 bg-amber-50 text-amber-900');

  function reset(): void {
    draft = createNewFileWorkspaceEditDraft();
  }

  function save(): void {
    if (!validation.request) {
      return;
    }
    void onSave?.(validation.request);
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5" data-testid="file-workspace-edit-form">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-base font-semibold text-admin-ink">New file workspace settings</h3>
      <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
        Prepare a reusable collection of files for sessions and workflow inputs.
      </p>
    </div>
    <span class={`inline-flex w-fit shrink-0 rounded-full border px-2.5 py-1 text-xs font-semibold ${changeStatusClass}`}>
      {changeStatusLabel}
    </span>
  </div>

  <div class="mt-5 grid gap-4">
    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="file-workspace-metadata-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Metadata</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Use concise names and labels so the workspace is easy to select from project and session configuration.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-2">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Name</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.name}
            disabled={disabled}
            autocomplete="off"
            data-testid="file-workspace-edit-name"
          />
          <FieldFeedback errors={validation.fieldErrors.name} testId="file-workspace-edit-name-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Labels</span>
          <textarea
            class="min-h-24 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            placeholder="team=support, purpose=input"
            bind:value={draft.labelsText}
            disabled={disabled}
            spellcheck="false"
            data-testid="file-workspace-edit-labels"
          ></textarea>
          <FieldFeedback
            errors={validation.fieldErrors.labels}
            hint="Use comma-separated or newline-separated key=value labels."
            testId="file-workspace-edit-labels-error"
          />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Description</span>
          <textarea
            class="min-h-24 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 text-sm leading-6 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.description}
            disabled={disabled}
            data-testid="file-workspace-edit-description"
          ></textarea>
          <FieldFeedback hint="Optional operator-facing explanation for this workspace." />
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="file-workspace-scope-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Scope</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Owner-scoped workspaces are reusable across projects. Project-scoped workspaces are only selectable by matching project sessions.
        </p>
      </div>

      {#if projectOptionsState.status === 'loading'}
        <div class="mt-4">
          <AdminMessage tone="loading" title="Loading projects" testId="file-workspace-projects-loading" />
        </div>
      {:else if projectOptionsState.status === 'error'}
        <div class="mt-4">
          <AdminMessage
            tone="warning"
            title="Project options unavailable"
            message={projectOptionsState.message}
            testId="file-workspace-projects-error"
          />
        </div>
      {/if}

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-[220px_minmax(0,1fr)]">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Binding</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.projectBinding}
            disabled={disabled}
            data-testid="file-workspace-edit-project-binding"
          >
            <option value="owner">owner scoped</option>
            <option value="project">project scoped</option>
          </select>
          <FieldFeedback hint="Choose whether this workspace is global or project-bound." />
        </label>

        {#if draft.projectBinding === 'project'}
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Project</span>
            <select
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              bind:value={draft.projectId}
              disabled={disabled}
              data-testid="file-workspace-edit-project-id"
            >
              <option value="">Select a project...</option>
              {#each projectOptions as project}
                <option value={project.id}>{project.name} · {project.state}</option>
              {/each}
            </select>
            <FieldFeedback errors={validation.fieldErrors.projectId} testId="file-workspace-edit-project-id-error" />
          </label>
        {:else}
          <p class="m-0 self-start rounded-md bg-admin-soft px-3 py-2 text-xs leading-5 text-admin-muted lg:mt-6">
            The API will persist `project_id=null`.
          </p>
        {/if}
      </div>
    </section>
  </div>

  <div class="sticky bottom-0 z-10 -mx-4 -mb-4 mt-5 flex flex-col gap-2 border-t border-admin-border bg-admin-panel/95 px-4 py-3 backdrop-blur sm:-mx-5 sm:-mb-5 sm:flex-row sm:items-center sm:justify-end sm:px-5">
    <button
      class="inline-flex h-10 w-full items-center justify-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
      type="button"
      onclick={reset}
      disabled={disabled || !changed}
      data-testid="file-workspace-edit-cancel"
    >
      Cancel
    </button>
    <button
      class="inline-flex h-10 w-full items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
      type="button"
      onclick={save}
      disabled={disabled || !changed || !validation.valid}
      data-testid="file-workspace-edit-save"
    >
      Create file workspace
    </button>
  </div>
</section>
