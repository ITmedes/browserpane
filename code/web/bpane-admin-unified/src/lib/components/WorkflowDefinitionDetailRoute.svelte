<script lang="ts">
  import { ArrowLeft } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import {
    WorkflowCatalogClient,
    WorkflowCatalogError,
  } from '$lib/workflows/workflow-client';
  import { WorkflowRunCatalogClient } from '$lib/workflow-runs/workflow-run-client';
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
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowDefinitionInspector from './WorkflowDefinitionInspector.svelte';

  type WorkflowDefinitionDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: WorkflowDefinitionDetailRouteProps = $props();
  let workflowState = $state<WorkflowDetailLoadState>({ status: 'idle' });
  let actionState = $state<WorkflowActionState>({ status: 'idle' });
  let sourceFilesState = $state<WorkflowSourceFileListState>({ status: 'idle' });
  let sourcePreviewState = $state<WorkflowSourcePreviewState>({ status: 'idle' });
  let sourceFilesRequestId = 0;
  let sourcePreviewRequestId = 0;

  onMount(() => {
    const workflowId = currentWorkflowId();
    if (!workflowId) {
      workflowState = {
        status: 'error',
        workflowId: 'unknown',
        message: 'Workflow id is missing from the current route.',
      };
      return;
    }
    void loadWorkflow(workflowId);
  });

  function client(): WorkflowCatalogClient {
    return new WorkflowCatalogClient(clientOptions());
  }

  function workflowRunClient(): WorkflowRunCatalogClient {
    return new WorkflowRunCatalogClient(clientOptions());
  }

  function clientOptions() {
    return {
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    };
  }

  async function loadWorkflow(workflowId: string): Promise<void> {
    workflowState = { status: 'loading', workflowId };
    actionState = { status: 'idle' };
    sourceFilesState = { status: 'idle' };
    sourcePreviewState = { status: 'idle' };
    try {
      const workflowClient = client();
      const [definition, versions] = await Promise.all([
        workflowClient.getDefinition(workflowId),
        workflowClient.listDefinitionVersions(workflowId),
      ]);
      workflowState = { status: 'ready', definition, versions: versions.versions };
    } catch (error) {
      workflowState = {
        status: 'error',
        workflowId,
        message: error instanceof Error ? error.message : 'Unexpected workflow definition detail error.',
      };
    }
  }

  async function refreshWorkflow(): Promise<void> {
    const workflowId = activeWorkflowId();
    if (!workflowId) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing workflow definition...' };
    try {
      const workflowClient = client();
      const [definition, versions] = await Promise.all([
        workflowClient.getDefinition(workflowId),
        workflowClient.listDefinitionVersions(workflowId),
      ]);
      workflowState = { status: 'ready', definition, versions: versions.versions };
      actionState = { status: 'success', message: 'Workflow definition refreshed.' };
      const previewVersion = activePreviewVersion();
      if (previewVersion) {
        void loadSourceFilesAndPreview(definition.id, previewVersion);
      }
    } catch (error) {
      actionState = {
        status: 'error',
        message: error instanceof Error ? error.message : 'Workflow definition refresh failed.',
      };
    }
  }

  async function selectWorkflowVersion(version: string): Promise<void> {
    const workflowId = activeWorkflowId();
    if (!workflowId) {
      return;
    }
    await loadSourceFilesAndPreview(workflowId, version);
  }

  async function loadSourceFilesAndPreview(workflowId: string, version: string): Promise<void> {
    const requestId = sourceFilesRequestId + 1;
    sourceFilesRequestId = requestId;
    sourcePreviewRequestId += 1;
    sourceFilesState = { status: 'loading', version };
    sourcePreviewState = { status: 'loading', version };
    try {
      const response = await client().listDefinitionSourceFiles(workflowId, version);
      if (sourceFilesRequestId !== requestId) {
        return;
      }
      sourceFilesState = { status: 'ready', version, response };
      const entrypointPath = response.files.find((file) => file.entrypoint)?.path ?? response.entrypoint;
      await loadSourcePreview(workflowId, version, entrypointPath);
    } catch (error) {
      if (sourceFilesRequestId !== requestId) {
        return;
      }
      const message = error instanceof Error ? error.message : 'Workflow source files failed.';
      if (error instanceof WorkflowCatalogError && error.status === 404) {
        const unavailableMessage = 'This workflow version does not expose source metadata for browsing.';
        sourceFilesState = { status: 'unavailable', version, message: unavailableMessage };
        sourcePreviewState = { status: 'unavailable', version, message: unavailableMessage };
        return;
      }
      sourceFilesState = { status: 'error', version, message };
      sourcePreviewState = { status: 'error', version, message };
    }
  }

  async function selectSourceFile(sourcePath: string): Promise<void> {
    const workflowId = activeWorkflowId();
    const version = activeSourceVersion();
    if (!workflowId || !version) {
      return;
    }
    await loadSourcePreview(workflowId, version, sourcePath);
  }

  async function validateWorkflowSource(
    request: ValidateWorkflowDefinitionSourceRequest,
  ): Promise<WorkflowDefinitionSourceValidationResponse> {
    const workflowId = activeWorkflowId();
    if (!workflowId) {
      throw new Error('Workflow id is not available for source validation.');
    }
    return await client().validateDefinitionSource(workflowId, request);
  }

  async function createWorkflowVersion(request: CreateWorkflowDefinitionVersionRequest): Promise<void> {
    const workflowId = activeWorkflowId();
    if (!workflowId) {
      throw new Error('Workflow id is not available for version creation.');
    }
    const workflowClient = client();
    const created = await workflowClient.createDefinitionVersion(workflowId, request);
    const [definition, versions] = await Promise.all([
      workflowClient.getDefinition(workflowId),
      workflowClient.listDefinitionVersions(workflowId),
    ]);
    workflowState = { status: 'ready', definition, versions: versions.versions };
    actionState = {
      status: 'success',
      message: `Workflow version ${created.version} created from validated source.`,
    };
    await loadSourceFilesAndPreview(workflowId, created.version);
  }

  async function startWorkflowRun(
    request: CreateWorkflowRunRequest,
    options: { readonly connectPreview: boolean },
  ): Promise<WorkflowRunLaunchResult> {
    const run = await workflowRunClient().createRun(request);
    if (!options.connectPreview) {
      return { run, previewOpened: false };
    }
    const previewOpened = openPreviewWindow(run.session_id);
    return { run, previewOpened, previewBlocked: !previewOpened };
  }

  async function loadSourcePreview(workflowId: string, version: string, sourcePath?: string): Promise<void> {
    const requestId = sourcePreviewRequestId + 1;
    sourcePreviewRequestId = requestId;
    sourcePreviewState = { status: 'loading', version };
    try {
      const preview = await client().getDefinitionSourcePreview(workflowId, version, sourcePath);
      if (sourcePreviewRequestId !== requestId) {
        return;
      }
      sourcePreviewState = { status: 'ready', version, preview };
    } catch (error) {
      if (sourcePreviewRequestId !== requestId) {
        return;
      }
      const message = error instanceof Error ? error.message : 'Workflow source preview failed.';
      if (error instanceof WorkflowCatalogError && error.status === 404) {
        sourcePreviewState = {
          status: 'unavailable',
          version,
          message: sourcePath
            ? 'This workflow source file is not available for preview.'
            : 'This workflow version does not expose source metadata for preview.',
        };
        return;
      }
      sourcePreviewState = { status: 'error', version, message };
    }
  }

  function currentWorkflowId(): string | null {
    const match = window.location.pathname.match(/\/workflows\/([^/]+)\/?$/);
    return match?.[1] ? decodeURIComponent(match[1]) : null;
  }

  function activeWorkflowId(): string | null {
    if (workflowState.status === 'ready') {
      return workflowState.definition.id;
    }
    if (workflowState.status === 'loading' || workflowState.status === 'error') {
      return workflowState.workflowId;
    }
    return null;
  }

  function activePreviewVersion(): string | null {
    if (sourcePreviewState.status === 'idle') {
      return null;
    }
    return sourcePreviewState.version;
  }

  function activeSourceVersion(): string | null {
    if (sourceFilesState.status !== 'idle') {
      return sourceFilesState.version;
    }
    return activePreviewVersion();
  }

  function openPreviewWindow(sessionId: string): boolean {
    const url = `/admin-new/sessions/${encodeURIComponent(sessionId)}/preview`;
    const popup = window.open(
      url,
      `bpane-session-preview-${sessionId}`,
      'popup=yes,width=1440,height=960,resizable=yes,scrollbars=no',
    );
    if (!popup) {
      return false;
    }
    popup.focus();
    return true;
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1180px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="workflow-definition-detail-route">
  <header class="flex flex-col gap-3 border-b border-admin-border pb-4 md:flex-row md:items-end md:justify-between">
    <div class="min-w-0">
      <a
        class="inline-flex items-center gap-2 text-sm font-medium text-admin-muted hover:text-admin-ink"
        href="/admin-new/workflows"
        data-testid="workflow-definition-detail-back"
      >
        <ArrowLeft size={16} strokeWidth={1.8} />
        <span>Workflows</span>
      </a>
      <p class="m-0 mt-3 text-xs font-semibold uppercase text-admin-muted">Operate</p>
      <h1 class="m-0 mt-1 text-2xl font-semibold text-admin-ink">Workflow definition details</h1>
    </div>
  </header>

  {#if workflowState.status === 'error'}
    <div data-testid="workflow-definition-detail-error">
      <AdminMessage tone="error" title="Workflow definition detail unavailable" message={workflowState.message} />
    </div>
  {:else}
    <WorkflowDefinitionInspector
      state={workflowState}
      {actionState}
      {sourceFilesState}
      {sourcePreviewState}
      onRefreshWorkflow={refreshWorkflow}
      onSelectVersion={selectWorkflowVersion}
      onSelectSourceFile={selectSourceFile}
      onValidateSource={validateWorkflowSource}
      onCreateVersion={createWorkflowVersion}
      onStartWorkflowRun={startWorkflowRun}
    />
  {/if}
</div>
