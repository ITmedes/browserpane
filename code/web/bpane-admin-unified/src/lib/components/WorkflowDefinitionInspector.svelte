<script lang="ts">
  import { Copy, RefreshCw } from '@lucide/svelte';
  import type {
    CreateWorkflowRunRequest,
    WorkflowRunLaunchResult,
  } from '$lib/workflow-runs/workflow-run-types';
  import type {
    CreateWorkflowDefinitionVersionRequest,
    ValidateWorkflowDefinitionSourceRequest,
    WorkflowDefinitionSourceValidationResponse,
    WorkflowDefinitionVersionResource,
  } from '$lib/workflows/workflow-types';
  import type {
    WorkflowActionState,
    WorkflowDetailLoadState,
    WorkflowSourceFileListState,
    WorkflowSourcePreviewState,
  } from '$lib/workflows/workflow-detail-state';
  import type { WorkflowRunProjectOptionsLoadState } from '$lib/workflows/workflow-run-launcher-view-model';
  import {
    buildWorkflowDefinitionDetailModel,
    labelSummary,
    type MetadataRow,
  } from '$lib/workflows/workflow-overview-view-model';
  import { formatDateTime } from '$lib/projects/project-formatters';
  import { projectToneClass } from '$lib/projects/project-ui';
  import AdminMessage from './AdminMessage.svelte';
  import ActionFeedback from './ActionFeedback.svelte';
  import WorkflowCodePreview from './WorkflowCodePreview.svelte';
  import WorkflowRunLauncher from './WorkflowRunLauncher.svelte';
  import WorkflowVersionSourceEditor from './WorkflowVersionSourceEditor.svelte';

  type WorkflowDefinitionInspectorProps = {
    readonly state: WorkflowDetailLoadState;
    readonly actionState?: WorkflowActionState;
    readonly sourceFilesState?: WorkflowSourceFileListState;
    readonly sourcePreviewState?: WorkflowSourcePreviewState;
    readonly projectOptionsState?: WorkflowRunProjectOptionsLoadState;
    readonly onRefreshWorkflow?: () => void | Promise<void>;
    readonly onSelectVersion?: (version: string) => void | Promise<void>;
    readonly onSelectSourceFile?: (path: string) => void | Promise<void>;
    readonly onValidateSource?: (
      request: ValidateWorkflowDefinitionSourceRequest,
    ) => Promise<WorkflowDefinitionSourceValidationResponse>;
    readonly onCreateVersion?: (request: CreateWorkflowDefinitionVersionRequest) => Promise<void>;
    readonly onStartWorkflowRun?: (
      request: CreateWorkflowRunRequest,
      options: { readonly connectPreview: boolean },
    ) => Promise<WorkflowRunLaunchResult>;
  };

  let {
    state: loadState,
    actionState = { status: 'idle' },
    sourceFilesState = { status: 'idle' },
    sourcePreviewState = { status: 'idle' },
    projectOptionsState = { status: 'idle' },
    onRefreshWorkflow,
    onSelectVersion,
    onSelectSourceFile,
    onValidateSource,
    onCreateVersion,
    onStartWorkflowRun,
  }: WorkflowDefinitionInspectorProps = $props();
  let selectedVersion = $state('');
  let notifiedVersion = $state('');

  const busy = $derived(actionState.status === 'running');
  const model = $derived(loadState.status === 'ready'
    ? buildWorkflowDefinitionDetailModel({
        definition: loadState.definition,
        versions: loadState.versions,
        selectedVersion,
      })
    : null);
  const selectedVersionResource = $derived(loadState.status === 'ready'
    ? selectedWorkflowVersionResource(loadState.versions, selectedVersion, loadState.definition.latest_version ?? null)
    : null);

  $effect(() => {
    if (loadState.status !== 'ready') {
      selectedVersion = '';
      notifiedVersion = '';
      return;
    }
    const nextVersion = selectedVersion && loadState.versions.some((version) => version.version === selectedVersion)
      ? selectedVersion
      : loadState.definition.latest_version ?? loadState.versions[0]?.version ?? '';
    if (selectedVersion !== nextVersion) {
      selectedVersion = nextVersion;
      return;
    }
    if (nextVersion && notifiedVersion !== nextVersion) {
      notifiedVersion = nextVersion;
      void onSelectVersion?.(nextVersion);
    }
  });

  $effect(() => {
    if (loadState.status !== 'ready' || sourcePreviewState.status === 'idle') {
      return;
    }
    const previewVersion = sourcePreviewState.version;
    if (
      previewVersion &&
      selectedVersion !== previewVersion &&
      loadState.versions.some((version) => version.version === previewVersion)
    ) {
      selectedVersion = previewVersion;
      notifiedVersion = previewVersion;
    }
  });

  function refreshWorkflow(): void {
    void onRefreshWorkflow?.();
  }

  async function copyWorkflowId(workflowId: string): Promise<void> {
    await navigator.clipboard?.writeText(workflowId);
  }

  function selectedWorkflowVersionResource(
    versions: readonly WorkflowDefinitionVersionResource[],
    selected: string,
    latestVersion: string | null,
  ): WorkflowDefinitionVersionResource | null {
    return versions.find((version) => version.version === selected)
      ?? versions.find((version) => version.version === latestVersion)
      ?? versions[0]
      ?? null;
  }
</script>

<aside class="min-w-0 rounded-md border border-admin-border bg-admin-panel" data-testid="workflow-definition-inspector">
  {#if loadState.status === 'idle'}
    <div class="flex min-h-64 items-center justify-center p-6 text-center text-sm text-admin-muted" data-testid="workflow-definition-inspector-idle">
      Select a workflow to inspect.
    </div>
  {:else if loadState.status === 'loading'}
    <div class="flex min-h-64 items-center justify-center p-6 text-sm text-admin-muted" data-testid="workflow-definition-inspector-loading">
      Loading workflow definition...
    </div>
  {:else if loadState.status === 'error'}
    <div class="p-4">
      <AdminMessage
        tone="error"
        title="Workflow definition unavailable"
        message={loadState.message}
        testId="workflow-definition-inspector-error"
      />
    </div>
  {:else if model}
    <div class="border-b border-admin-border p-4">
      <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div class="min-w-0">
          <div class="flex min-w-0 flex-wrap items-center gap-2">
            <h2 class="m-0 min-w-0 max-w-full truncate text-xl font-semibold text-admin-ink" data-testid="workflow-definition-detail-title">{model.name}</h2>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(model.kind === 'Example template' ? 'success' : model.kind === 'Template' ? 'warning' : 'neutral')}`} data-testid="workflow-definition-detail-kind">
              {model.kind}
            </span>
            <span class={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${projectToneClass(loadState.definition.latest_version ? 'success' : 'warning')}`} data-testid="workflow-definition-detail-latest-version">
              {model.latestVersion}
            </span>
          </div>
          <p class="m-0 mt-1 text-sm text-admin-muted">{model.description}</p>
          <p class="m-0 mt-2 min-w-0 truncate font-mono text-xs text-admin-muted">{model.definitionId}</p>
        </div>

        <div class="flex shrink-0 flex-wrap gap-2">
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft"
            type="button"
            onclick={() => void copyWorkflowId(model.definitionId)}
            data-testid="workflow-definition-copy-id"
          >
            <Copy size={15} strokeWidth={1.8} />
            <span>Copy ID</span>
          </button>
          <button
            class="inline-flex h-9 items-center gap-2 rounded-md border border-admin-border bg-admin-panel px-3 text-sm font-medium text-admin-ink hover:bg-admin-soft disabled:cursor-not-allowed disabled:opacity-60"
            type="button"
            onclick={refreshWorkflow}
            disabled={busy}
            data-testid="workflow-definition-refresh-detail"
          >
            <RefreshCw size={15} strokeWidth={1.8} />
            <span>Refresh</span>
          </button>
        </div>
      </div>
    </div>

    <div class="border-b border-admin-border p-4">
      <ActionFeedback
        state={actionState}
        successTitle="Workflow action completed"
        errorTitle="Workflow action failed"
        successTestId="workflow-definition-action-success"
        errorTestId="workflow-definition-action-error"
        runningTestId="workflow-definition-action-running"
      />
    </div>

    <div class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 p-4">
      <section class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="workflow-definition-summary">
        <div class="border-b border-admin-border pb-3">
          <h3 class="m-0 text-sm font-semibold text-admin-ink">Definition</h3>
          <p class="m-0 mt-1 text-xs leading-5 text-admin-muted">
            Workflow metadata is safe to inspect. Secrets and produced files are resolved only from run-specific views.
          </p>
        </div>

        <dl class="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Workflow id</dt>
            <dd class="m-0 mt-1 break-all font-mono text-xs text-admin-ink">{model.definitionId}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Versions</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink" data-testid="workflow-definition-detail-version-count">{model.versionRows.length}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Updated</dt>
            <dd class="m-0 mt-1 text-sm font-medium text-admin-ink">{formatDateTime(loadState.definition.updated_at)}</dd>
          </div>
          <div class="min-w-0 rounded-md border border-admin-border bg-admin-panel p-3 md:col-span-2 xl:col-span-3">
            <dt class="text-xs font-semibold uppercase text-admin-muted">Labels</dt>
            <dd class="m-0 mt-1 min-w-0 break-words text-sm font-medium text-admin-ink" data-testid="workflow-definition-detail-labels">{labelSummary(loadState.definition.labels)}</dd>
          </div>
        </dl>
      </section>

      <section class="grid min-w-0 gap-4 lg:grid-cols-[minmax(220px,320px)_minmax(0,1fr)]">
        <div class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="workflow-definition-versions-panel">
          <div class="border-b border-admin-border pb-3">
            <h3 class="m-0 text-sm font-semibold text-admin-ink">Versions</h3>
          </div>
          {#if model.versionRows.length === 0}
            <p class="m-0 mt-4 rounded-md border border-dashed border-admin-border bg-admin-panel p-4 text-sm text-admin-muted" data-testid="workflow-definition-versions-empty">
              No workflow versions are published yet.
            </p>
          {:else}
            <div class="mt-4 grid gap-2">
              {#each model.versionRows as version}
                <button
                  class={`grid min-w-0 gap-1 rounded-md border p-3 text-left text-sm ${
                    selectedVersion === version.version
                      ? 'border-admin-accent bg-admin-panel text-admin-ink shadow-sm'
                      : 'border-admin-border bg-admin-panel text-admin-muted hover:bg-admin-soft'
                  }`}
                  type="button"
                  data-testid="workflow-definition-version-row"
                  data-version={version.version}
                  onclick={() => {
                    selectedVersion = version.version;
                  }}
                >
                  <span class="flex min-w-0 items-center justify-between gap-2">
                    <strong class="truncate">{version.version}</strong>
                    {#if version.latest}
                      <span class="rounded-full bg-emerald-50 px-2 py-0.5 text-xs font-semibold text-emerald-700 ring-1 ring-emerald-200">latest</span>
                    {/if}
                  </span>
                  <span class="min-w-0 truncate text-xs">{version.executor} · {version.createdAt}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <div class="min-w-0 rounded-md border border-admin-border bg-admin-soft/50 p-4" data-testid="workflow-definition-version-panel">
          <div class="flex flex-col gap-3 border-b border-admin-border pb-3 sm:flex-row sm:items-start sm:justify-between">
            <div class="min-w-0">
              <h3 class="m-0 text-sm font-semibold text-admin-ink" data-testid="workflow-definition-selected-version">
                {model.selectedVersion?.version ?? 'No version selected'}
              </h3>
              {#if model.selectedVersion}
                <p class="m-0 mt-1 break-words text-xs leading-5 text-admin-muted" data-testid="workflow-definition-version-entrypoint">
                  {model.selectedVersion.entrypoint}
                </p>
              {/if}
            </div>
            {#if model.selectedVersion}
              <span class="inline-flex w-fit rounded-full bg-admin-panel px-2 py-0.5 text-xs font-semibold text-admin-ink ring-1 ring-admin-border" data-testid="workflow-definition-version-executor">
                {model.selectedVersion.executor}
              </span>
            {/if}
          </div>

          {#if model.selectedVersion}
            <div class="mt-4 grid min-w-0 gap-3 xl:grid-cols-3">
              {@render MetadataPanel('Source', model.selectedVersion.sourceRows, 'workflow-definition-source')}
              {@render MetadataPanel('Policy', model.selectedVersion.policyRows, 'workflow-definition-policy')}
              {@render MetadataPanel('Schemas', model.selectedVersion.schemaRows, 'workflow-definition-schemas')}
            </div>
          {:else}
            <AdminMessage tone="empty" title="No version metadata" message="Publish a workflow version before this definition can be invoked." density="compact" />
          {/if}
        </div>
      </section>

      <WorkflowRunLauncher
        workflowId={loadState.definition.id}
        selectedVersion={selectedVersionResource}
        {projectOptionsState}
        disabled={busy}
        onStartRun={onStartWorkflowRun}
      />

      <WorkflowVersionSourceEditor
        versions={loadState.versions}
        baseVersion={selectedVersionResource}
        disabled={busy}
        onValidateSource={onValidateSource}
        onCreateVersion={onCreateVersion}
      />

      <WorkflowCodePreview
        state={sourcePreviewState}
        filesState={sourceFilesState}
        onSelectFile={onSelectSourceFile}
      />
    </div>
  {/if}
</aside>

{#snippet MetadataPanel(title: string, rows: readonly MetadataRow[], testId: string)}
  <section class="grid min-w-0 gap-2 rounded-md border border-admin-border bg-admin-panel p-3" data-testid={testId}>
    <h4 class="m-0 text-sm font-semibold text-admin-ink">{title}</h4>
    {#each rows as row}
      <p class="m-0 min-w-0 text-xs leading-5 text-admin-muted">
        <strong class="block text-admin-ink">{row.label}</strong>
        <span class="break-words">{row.value}</span>
      </p>
    {/each}
  </section>
{/snippet}
