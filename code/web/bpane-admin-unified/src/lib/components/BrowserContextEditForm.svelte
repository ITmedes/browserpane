<script lang="ts">
  import {
    createNewBrowserContextEditDraft,
    hasBrowserContextEditChanges,
    mergeProjectsWithSelected,
    validateBrowserContextEdit,
    type BrowserContextEditDraft,
  } from '$lib/browser-contexts/browser-context-edit-view-model';
  import type { BrowserContextProjectOptionsLoadState } from '$lib/browser-contexts/browser-context-detail-state';
  import type { CreateBrowserContextRequest } from '$lib/browser-contexts/browser-context-types';
  import AdminMessage from './AdminMessage.svelte';
  import FieldFeedback from './FieldFeedback.svelte';

  type BrowserContextEditFormProps = {
    readonly disabled?: boolean;
    readonly initialDraft?: BrowserContextEditDraft;
    readonly title?: string;
    readonly description?: string;
    readonly submitLabel?: string;
    readonly persistenceLocked?: boolean;
    readonly requireChanges?: boolean;
    readonly submitBlocked?: boolean;
    readonly submitBlockedHint?: string | null;
    readonly projectOptionsState?: BrowserContextProjectOptionsLoadState;
    readonly onSave?: (request: CreateBrowserContextRequest) => void | Promise<void>;
  };

  let {
    disabled = false,
    initialDraft = createNewBrowserContextEditDraft(),
    title = 'New browser context settings',
    description = 'Define scope, persistence, retention, and profile-storage limits before creating the reusable context.',
    submitLabel = 'Create browser context',
    persistenceLocked = false,
    requireChanges = true,
    submitBlocked = false,
    submitBlockedHint = null,
    projectOptionsState = { status: 'idle' },
    onSave,
  }: BrowserContextEditFormProps = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state<BrowserContextEditDraft>({ ...initialDraft });

  const validation = $derived(validateBrowserContextEdit(draft));
  const changed = $derived(hasBrowserContextEditChanges(draft, initialDraft));
  const readyToSubmit = $derived(
    validation.valid && !submitBlocked && (!requireChanges || changed),
  );
  const changeStatusLabel = $derived(disabled
    ? 'Read-only access'
    : readyToSubmit
      ? 'Ready to submit'
      : 'Context draft');
  const changeStatusClass = $derived(disabled
    ? 'border-admin-border bg-admin-soft text-admin-muted'
    : readyToSubmit
      ? 'border-emerald-200 bg-emerald-50 text-emerald-800'
      : 'border-amber-200 bg-amber-50 text-amber-900');
  const projectOptions = $derived(projectOptionsState.status === 'ready'
    ? mergeProjectsWithSelected(projectOptionsState.projects, draft.projectId)
    : mergeProjectsWithSelected([], draft.projectId));

  function reset(): void {
    draft = { ...initialDraft };
  }

  function save(): void {
    if (!validation.request) {
      return;
    }
    void onSave?.(validation.request);
  }

  function checked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-panel p-4 sm:p-5" data-testid="browser-context-edit-form">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-4 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-base font-semibold text-admin-ink">{title}</h3>
      <p class="m-0 mt-1 text-sm leading-6 text-admin-muted">
        {description}
      </p>
    </div>
    <span class={`inline-flex w-fit shrink-0 rounded-full border px-2.5 py-1 text-xs font-semibold ${changeStatusClass}`}>
      {changeStatusLabel}
    </span>
  </div>

  <div class="mt-5 grid gap-4">
    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="browser-context-metadata-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Metadata</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Keep the context identity clear for operators selecting reusable browser profiles.
        </p>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Name</span>
          <input
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            type="text"
            bind:value={draft.name}
            disabled={disabled}
            autocomplete="off"
            data-testid="browser-context-edit-name"
          />
          <FieldFeedback errors={validation.fieldErrors.name} testId="browser-context-edit-name-error" />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm">
          <span class="font-medium text-admin-ink">Persistence</span>
          <select
            class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.persistenceMode}
            disabled={disabled || persistenceLocked}
            data-testid="browser-context-edit-persistence-mode"
          >
            <option value="reusable">reusable</option>
            <option value="ephemeral">ephemeral</option>
          </select>
          <FieldFeedback hint={persistenceLocked
            ? 'This lifecycle operation always creates a reusable context.'
            : 'Reusable contexts can be selected by future sessions.'} />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Description</span>
          <textarea
            class="min-h-24 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 text-sm leading-6 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            bind:value={draft.description}
            disabled={disabled}
            data-testid="browser-context-edit-description"
          ></textarea>
          <FieldFeedback hint="Optional operator-facing explanation for this browser context." />
        </label>

        <label class="grid min-w-0 content-start gap-1.5 text-sm lg:col-span-2">
          <span class="font-medium text-admin-ink">Labels</span>
          <textarea
            class="min-h-28 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 py-2 font-mono text-xs leading-5 text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
            placeholder="team=support&#10;profile=baseline"
            bind:value={draft.labelsText}
            disabled={disabled}
            spellcheck="false"
            data-testid="browser-context-edit-labels"
          ></textarea>
          <FieldFeedback
            errors={validation.fieldErrors.labels}
            hint="One key=value label per line."
            testId="browser-context-edit-labels-error"
          />
        </label>
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="browser-context-scope-editor">
      <div class="border-b border-admin-border pb-3">
        <h4 class="m-0 text-sm font-semibold text-admin-ink">Scope</h4>
        <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
          Owner-scoped contexts are reusable across projects. Project-scoped contexts are only usable by matching sessions.
        </p>
      </div>

      {#if projectOptionsState.status === 'loading'}
        <div class="mt-4">
          <AdminMessage tone="loading" title="Loading projects" testId="browser-context-projects-loading" />
        </div>
      {:else if projectOptionsState.status === 'error'}
        <div class="mt-4">
          <AdminMessage
            tone="warning"
            title="Project options unavailable"
            message={projectOptionsState.message}
            testId="browser-context-projects-error"
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
            data-testid="browser-context-edit-project-binding"
          >
            <option value="owner">owner scoped</option>
            <option value="project">project scoped</option>
          </select>
          <FieldFeedback hint="Choose whether this context is reusable globally or bound to one project." />
        </label>

        {#if draft.projectBinding === 'project'}
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Project</span>
            <select
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              bind:value={draft.projectId}
              disabled={disabled}
              data-testid="browser-context-edit-project-id"
            >
              <option value="">Select a project...</option>
              {#each projectOptions as project}
                <option value={project.id}>{project.name} · {project.state}</option>
              {/each}
            </select>
            <FieldFeedback errors={validation.fieldErrors.projectId} testId="browser-context-edit-project-id-error" />
          </label>
        {:else}
          <p class="m-0 self-start rounded-md bg-admin-soft px-3 py-2 text-xs leading-5 text-admin-muted lg:mt-6">
            The API will persist `project_id=null`.
          </p>
        {/if}
      </div>
    </section>

    <section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="browser-context-retention-editor">
      <div class="flex flex-col gap-3 border-b border-admin-border pb-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="min-w-0">
          <h4 class="m-0 text-sm font-semibold text-admin-ink">Retention and storage</h4>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
            Control how long inactive reusable contexts are retained and when profile storage should block new reuse.
          </p>
        </div>
      </div>

      <div class="mt-4 grid items-start gap-4 lg:grid-cols-2">
        <div class="grid min-w-0 gap-2 rounded-md border border-admin-border bg-admin-panel p-3">
          <label class="inline-flex items-center gap-2 text-sm font-medium text-admin-ink">
            <input
              class="h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
              type="checkbox"
              checked={draft.retentionEnabled}
              disabled={disabled}
              onchange={(event) => {
                draft = { ...draft, retentionEnabled: checked(event) };
              }}
              data-testid="browser-context-edit-retention-enabled"
            />
            <span>Retention window</span>
          </label>
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Seconds</span>
            <input
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              type="number"
              min="1"
              step="1"
              bind:value={draft.retentionSec}
              disabled={disabled || !draft.retentionEnabled}
              data-testid="browser-context-edit-retention-sec"
            />
            <FieldFeedback
              errors={validation.fieldErrors.retentionSec}
              hint="604800 seconds equals 7 days."
              testId="browser-context-edit-retention-sec-error"
            />
          </label>
        </div>

        <div class="grid min-w-0 gap-2 rounded-md border border-admin-border bg-admin-panel p-3">
          <label class="inline-flex items-center gap-2 text-sm font-medium text-admin-ink">
            <input
              class="h-4 w-4 rounded border-admin-border text-admin-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 disabled:cursor-not-allowed disabled:opacity-60"
              type="checkbox"
              checked={draft.storageLimitEnabled}
              disabled={disabled}
              onchange={(event) => {
                draft = { ...draft, storageLimitEnabled: checked(event) };
              }}
              data-testid="browser-context-edit-storage-limit-enabled"
            />
            <span>Profile storage limit</span>
          </label>
          <label class="grid min-w-0 content-start gap-1.5 text-sm">
            <span class="font-medium text-admin-ink">Bytes</span>
            <input
              class="h-10 w-full min-w-0 rounded-md border border-admin-border bg-white px-3 font-mono text-xs text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted"
              type="number"
              min="1"
              step="1"
              bind:value={draft.maxProfileStorageBytes}
              disabled={disabled || !draft.storageLimitEnabled}
              data-testid="browser-context-edit-max-profile-storage-bytes"
            />
            <FieldFeedback
              errors={validation.fieldErrors.maxProfileStorageBytes}
              hint="Optional hard limit checked before reusable sessions start."
              testId="browser-context-edit-max-profile-storage-bytes-error"
            />
          </label>
        </div>
      </div>
    </section>
  </div>

  {#if submitBlockedHint}
    <div class="mt-4">
      <AdminMessage
        tone="info"
        title="Additional input required"
        message={submitBlockedHint}
        testId="browser-context-edit-submit-blocked"
      />
    </div>
  {/if}

  <div class="sticky bottom-0 z-10 -mx-4 -mb-4 mt-5 flex flex-col gap-2 border-t border-admin-border bg-admin-panel/95 px-4 py-3 backdrop-blur sm:-mx-5 sm:-mb-5 sm:flex-row sm:items-center sm:justify-end sm:px-5">
    <button
      class="inline-flex h-10 w-full items-center justify-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
      type="button"
      onclick={reset}
      disabled={disabled || !changed}
      data-testid="browser-context-edit-cancel"
    >
      Cancel
    </button>
    <button
      class="inline-flex h-10 w-full items-center justify-center rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-admin-accent/30 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60 sm:w-auto"
      type="button"
      onclick={save}
      disabled={disabled || !readyToSubmit}
      data-testid="browser-context-edit-save"
    >
      {submitLabel}
    </button>
  </div>
</section>
