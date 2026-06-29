<script lang="ts">
  import { GitBranch, Plus, ShieldCheck } from '@lucide/svelte';
  import {
    createWorkflowSourceEditorDraft,
    validateWorkflowSourceEditorDraft,
    workflowSourceEditorRequestKey,
    type WorkflowSourceEditorDraft,
  } from '$lib/workflows/workflow-source-editor-view-model';
  import type {
    CreateWorkflowDefinitionVersionRequest,
    ValidateWorkflowDefinitionSourceRequest,
    WorkflowDefinitionSourceValidationResponse,
    WorkflowDefinitionVersionResource,
  } from '$lib/workflows/workflow-types';
  import AdminMessage from './AdminMessage.svelte';
  import FieldFeedback from './FieldFeedback.svelte';
  import WorkflowSourceTree from './WorkflowSourceTree.svelte';

  type ValidationState =
    | { readonly status: 'idle' }
    | { readonly status: 'running'; readonly requestKey: string }
    | {
        readonly status: 'ready';
        readonly requestKey: string;
        readonly response: WorkflowDefinitionSourceValidationResponse;
      }
    | { readonly status: 'error'; readonly requestKey: string; readonly message: string };

  type WorkflowVersionSourceEditorProps = {
    readonly versions: readonly WorkflowDefinitionVersionResource[];
    readonly baseVersion?: WorkflowDefinitionVersionResource | null;
    readonly disabled?: boolean;
    readonly onValidateSource?: (
      request: ValidateWorkflowDefinitionSourceRequest,
    ) => Promise<WorkflowDefinitionSourceValidationResponse>;
    readonly onCreateVersion?: (request: CreateWorkflowDefinitionVersionRequest) => Promise<void>;
  };

  let {
    versions,
    baseVersion = null,
    disabled = false,
    onValidateSource,
    onCreateVersion,
  }: WorkflowVersionSourceEditorProps = $props();

  // svelte-ignore state_referenced_locally
  let draft = $state<WorkflowSourceEditorDraft>(createWorkflowSourceEditorDraft(versions, baseVersion));
  let validationState = $state<ValidationState>({ status: 'idle' });
  let createState = $state<{ status: 'idle' } | { status: 'running' } | { status: 'error'; message: string }>({
    status: 'idle',
  });

  const validation = $derived(validateWorkflowSourceEditorDraft(draft, versions));
  const requestKey = $derived(workflowSourceEditorRequestKey(validation.sourceRequest));
  const validationReady = $derived(
    validationState.status === 'ready' && validationState.requestKey === requestKey,
  );
  const validating = $derived(validationState.status === 'running');
  const creating = $derived(createState.status === 'running');
  const canValidate = $derived(Boolean(onValidateSource) && !disabled && !validating && !creating && validation.valid);
  const canCreate = $derived(
    Boolean(onCreateVersion) && !disabled && !validating && !creating && validation.valid && validationReady,
  );
  const resolvedCommit = $derived(
    validationState.status === 'ready'
      ? validationState.response.source.resolved_commit ?? 'unresolved'
      : 'Validate source to pin commit',
  );

  async function validateSource(): Promise<void> {
    if (!validation.sourceRequest || !onValidateSource) {
      return;
    }
    validationState = { status: 'running', requestKey };
    createState = { status: 'idle' };
    try {
      const response = await onValidateSource(validation.sourceRequest);
      validationState = { status: 'ready', requestKey, response };
    } catch (error) {
      validationState = {
        status: 'error',
        requestKey,
        message: error instanceof Error ? error.message : 'Workflow source validation failed.',
      };
    }
  }

  async function createVersion(): Promise<void> {
    if (!validation.request || !canCreate || !onCreateVersion) {
      return;
    }
    createState = { status: 'running' };
    try {
      const validatedSource = validationState.status === 'ready'
        ? validationState.response.source
        : validation.request.source;
      await onCreateVersion({
        ...validation.request,
        source: validatedSource,
      });
      createState = { status: 'idle' };
      validationState = { status: 'idle' };
      draft = createWorkflowSourceEditorDraft(
        [
          ...versions,
          {
            id: `pending-${validation.request.version}`,
            workflow_definition_id: '',
            version: validation.request.version,
            executor: validation.request.executor,
            entrypoint: validation.request.entrypoint,
            source: validatedSource,
            allowed_credential_binding_ids: [],
            allowed_extension_ids: [],
            allowed_file_workspace_ids: [],
            created_at: new Date().toISOString(),
          },
        ],
        null,
      );
    } catch (error) {
      createState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Workflow version creation failed.',
      };
    }
  }

  function resetDraft(): void {
    draft = createWorkflowSourceEditorDraft(versions, baseVersion);
    validationState = { status: 'idle' };
    createState = { status: 'idle' };
  }

  function setEntrypoint(path: string): void {
    draft = { ...draft, entrypoint: path };
  }

  function validationFieldClass(hasError: boolean): string {
    return `h-10 rounded-md border bg-white px-3 text-sm text-admin-ink outline-none transition focus:border-admin-accent focus-visible:ring-2 focus-visible:ring-admin-accent/25 disabled:cursor-not-allowed disabled:bg-admin-soft disabled:text-admin-muted ${
      hasError ? 'border-red-300' : 'border-admin-border'
    }`;
  }
</script>

<section class="rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="workflow-source-editor">
  <div class="flex flex-col gap-3 border-b border-admin-border pb-3 lg:flex-row lg:items-start lg:justify-between">
    <div class="min-w-0">
      <h3 class="m-0 text-sm font-semibold text-admin-ink">New version source</h3>
      <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
        Validate a Git source, pin the resolved commit, then publish it as a new immutable workflow version.
      </p>
    </div>
    <span class="inline-flex w-fit shrink-0 items-center gap-1 rounded-full border border-admin-border bg-admin-panel px-2.5 py-1 text-xs font-semibold text-admin-ink">
      <GitBranch size={13} strokeWidth={1.8} />
      <span>{resolvedCommit}</span>
    </span>
  </div>

  <div class="mt-4 grid gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(280px,360px)]">
    <div class="grid gap-4">
      <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_160px]">
        <label class="grid gap-1 text-sm">
          <span class="font-medium text-admin-ink">Repository URL</span>
          <input
            class={validationFieldClass(Boolean(validation.fieldErrors.repositoryUrl?.length))}
            type="text"
            bind:value={draft.repositoryUrl}
            disabled={disabled || creating}
            autocomplete="off"
            data-testid="workflow-source-repository-url"
          />
          <FieldFeedback
            errors={validation.fieldErrors.repositoryUrl}
            hint="Use /workspace locally or a reachable Git HTTPS/SSH URL."
            testId="workflow-source-repository-url-error"
          />
        </label>

        <label class="grid gap-1 text-sm">
          <span class="font-medium text-admin-ink">Ref</span>
          <input
            class={validationFieldClass(Boolean(validation.fieldErrors.ref?.length))}
            type="text"
            bind:value={draft.ref}
            disabled={disabled || creating}
            autocomplete="off"
            data-testid="workflow-source-ref"
          />
          <FieldFeedback
            errors={validation.fieldErrors.ref}
            hint="Branch, tag, or HEAD."
            testId="workflow-source-ref-error"
          />
        </label>
      </div>

      <div class="grid gap-4 md:grid-cols-[120px_150px_minmax(0,1fr)]">
        <label class="grid gap-1 text-sm">
          <span class="font-medium text-admin-ink">Version</span>
          <input
            class={validationFieldClass(Boolean(validation.fieldErrors.version?.length))}
            type="text"
            bind:value={draft.version}
            disabled={disabled || creating}
            autocomplete="off"
            data-testid="workflow-source-version"
          />
          <FieldFeedback errors={validation.fieldErrors.version} testId="workflow-source-version-error" />
        </label>

        <label class="grid gap-1 text-sm">
          <span class="font-medium text-admin-ink">Executor</span>
          <input
            class={validationFieldClass(Boolean(validation.fieldErrors.executor?.length))}
            type="text"
            bind:value={draft.executor}
            disabled={disabled || creating}
            autocomplete="off"
            data-testid="workflow-source-executor"
          />
          <FieldFeedback errors={validation.fieldErrors.executor} testId="workflow-source-executor-error" />
        </label>

        <label class="grid gap-1 text-sm">
          <span class="font-medium text-admin-ink">Root path</span>
          <input
            class={validationFieldClass(false)}
            type="text"
            bind:value={draft.rootPath}
            disabled={disabled || creating}
            autocomplete="off"
            data-testid="workflow-source-root-path"
          />
          <FieldFeedback hint="Optional relative path that limits the source tree." />
        </label>
      </div>

      <label class="grid gap-1 text-sm">
        <span class="font-medium text-admin-ink">Entrypoint</span>
        <input
          class={validationFieldClass(Boolean(validation.fieldErrors.entrypoint?.length))}
          type="text"
          bind:value={draft.entrypoint}
          disabled={disabled || creating}
          autocomplete="off"
          data-testid="workflow-source-entrypoint"
        />
        <FieldFeedback
          errors={validation.fieldErrors.entrypoint}
          hint="Must be a relative file path under the root path."
          testId="workflow-source-entrypoint-error"
        />
      </label>
    </div>

    <aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3" data-testid="workflow-source-validation-panel">
      {#if validationState.status === 'idle'}
        <AdminMessage
          tone="info"
          density="compact"
          title="Source not validated"
          message="Validate before creating the workflow version."
          testId="workflow-source-validation-idle"
        />
      {:else if validationState.status === 'running'}
        <AdminMessage
          tone="loading"
          density="compact"
          title="Validating source..."
          message={draft.repositoryUrl}
          testId="workflow-source-validation-running"
        />
      {:else if validationState.status === 'error'}
        <AdminMessage
          tone="error"
          density="compact"
          title="Source validation failed"
          message={validationState.message}
          testId="workflow-source-validation-error"
        />
      {:else}
        <div data-testid="workflow-source-validation-ready">
          <AdminMessage
            tone={validationReady ? 'success' : 'warning'}
            density="compact"
            title={validationReady ? 'Source validated' : 'Validation is stale'}
            message={`${validationState.response.files.length} files available at ${validationState.response.source.resolved_commit ?? 'unresolved commit'}.`}
          />
          <WorkflowSourceTree
            files={validationState.response.files}
            selectedPath={draft.entrypoint}
            onSelectFile={setEntrypoint}
          />
        </div>
      {/if}
    </aside>
  </div>

  {#if createState.status === 'error'}
    <div class="mt-4">
      <AdminMessage
        tone="error"
        density="compact"
        title="Workflow version creation failed"
        message={createState.message}
        testId="workflow-source-create-error"
      />
    </div>
  {/if}

  <div class="mt-4 flex flex-col gap-2 border-t border-admin-border pt-3 sm:flex-row sm:items-center sm:justify-end">
    <button
      class="inline-flex h-10 items-center justify-center rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={resetDraft}
      disabled={disabled || validating || creating}
      data-testid="workflow-source-reset"
    >
      Reset
    </button>
    <button
      class="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink transition hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void validateSource()}
      disabled={!canValidate}
      data-testid="workflow-source-validate"
    >
      <ShieldCheck size={15} strokeWidth={1.8} />
      <span>Validate source</span>
    </button>
    <button
      class="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-admin-accent bg-admin-accent px-3 text-sm font-semibold text-white shadow-sm transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
      type="button"
      onclick={() => void createVersion()}
      disabled={!canCreate}
      data-testid="workflow-source-create-version"
    >
      <Plus size={15} strokeWidth={1.8} />
      <span>{creating ? 'Creating...' : 'Create version'}</span>
    </button>
  </div>
</section>
