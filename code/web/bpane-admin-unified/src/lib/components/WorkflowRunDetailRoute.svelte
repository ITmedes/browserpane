<script lang="ts">
  import { onMount } from 'svelte';
  import type { AdminActionState } from '$lib/application/admin-async-state';
  import { adminErrorMessage } from '$lib/application/admin-async-state';
  import type { UnifiedAdminContext } from '$lib/auth/unified-admin-context';
  import {
    workflowRunIdFromPathname,
    type WorkflowRunDetailEvidenceState,
    type WorkflowRunDetailLoadState,
    type WorkflowRunEvidenceState,
  } from '$lib/workflow-runs/workflow-run-detail-view-model';
  import { WorkflowRunCatalogClient } from '$lib/workflow-runs/workflow-run-client';
  import type {
    WorkflowRunEventResource,
    WorkflowRunLogResource,
    WorkflowRunProducedFileResource,
    WorkflowRunResource,
  } from '$lib/workflow-runs/workflow-run-types';
  import AdminMessage from './AdminMessage.svelte';
  import WorkflowRunInspector from './WorkflowRunInspector.svelte';

  type WorkflowRunDetailRouteProps = {
    readonly authContext: UnifiedAdminContext;
  };

  let { authContext }: WorkflowRunDetailRouteProps = $props();
  let detailState = $state<WorkflowRunDetailLoadState>({ status: 'loading', runId: 'unknown' });
  let evidenceState = $state<WorkflowRunDetailEvidenceState>(emptyEvidenceState());
  let actionState = $state<AdminActionState>({ status: 'idle' });
  let downloadState = $state<AdminActionState>({ status: 'idle' });

  onMount(() => {
    const runId = workflowRunIdFromPathname(window.location.pathname);
    if (!runId) {
      detailState = {
        status: 'error',
        runId: 'unknown',
        message: 'Workflow run id is missing or invalid in the current route.',
      };
      return;
    }
    void loadRun(runId);
  });

  function client(): WorkflowRunCatalogClient {
    return new WorkflowRunCatalogClient({
      baseUrl: window.location.origin,
      accessTokenProvider: authContext.accessTokenProvider,
      onAuthenticationFailure: authContext.onAuthenticationFailure,
    });
  }

  async function loadRun(runId: string): Promise<void> {
    detailState = { status: 'loading', runId };
    actionState = { status: 'idle' };
    downloadState = { status: 'idle' };
    try {
      const run = await client().getRun(runId);
      detailState = { status: 'ready', run };
      await loadEvidence(run.id);
    } catch (error) {
      detailState = {
        status: 'error',
        runId,
        message: adminErrorMessage(error, 'Unexpected workflow run detail error.'),
      };
    }
  }

  async function refreshRun(): Promise<void> {
    const run = currentRun();
    if (!run) {
      return;
    }
    actionState = { status: 'running', label: 'Refreshing workflow run...' };
    try {
      const refreshed = await client().getRun(run.id);
      detailState = { status: 'ready', run: refreshed };
      await loadEvidence(refreshed.id);
      actionState = { status: 'success', message: 'Workflow run and evidence refreshed.' };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Workflow run refresh failed.'),
      };
    }
  }

  async function cancelRun(): Promise<void> {
    await mutateRun(
      'Cancelling workflow run...',
      'Workflow run cancellation requested.',
      (runClient, runId) => runClient.cancelRun(runId),
    );
  }

  async function resumeRun(): Promise<void> {
    await mutateRun(
      'Resuming workflow run...',
      'Workflow run resumed.',
      (runClient, runId) => runClient.resumeRun(runId, {
        comment: 'Resumed from admin-new workflow run detail.',
      }),
    );
  }

  async function submitInput(input: unknown): Promise<void> {
    await mutateRun(
      'Submitting operator input...',
      'Operator input submitted.',
      (runClient, runId) => runClient.submitRunInput(runId, {
        input,
        comment: 'Submitted from admin-new workflow run detail.',
      }),
    );
  }

  async function rejectRun(reason: string): Promise<void> {
    await mutateRun(
      'Rejecting intervention request...',
      'Intervention request rejected.',
      (runClient, runId) => runClient.rejectRun(runId, { reason }),
    );
  }

  async function mutateRun(
    runningLabel: string,
    successMessage: string,
    action: (
      runClient: WorkflowRunCatalogClient,
      runId: string,
    ) => Promise<WorkflowRunResource>,
  ): Promise<void> {
    const run = currentRun();
    if (!run) {
      return;
    }
    actionState = { status: 'running', label: runningLabel };
    try {
      const updated = await action(client(), run.id);
      detailState = { status: 'ready', run: updated };
      await loadEvidence(updated.id);
      actionState = { status: 'success', message: successMessage };
    } catch (error) {
      actionState = {
        status: 'error',
        message: adminErrorMessage(error, 'Workflow run action failed.'),
      };
    }
  }

  async function loadEvidence(runId: string): Promise<void> {
    evidenceState = {
      events: { status: 'loading' },
      logs: { status: 'loading' },
      producedFiles: { status: 'loading' },
    };
    const runClient = client();
    const [events, logs, producedFiles] = await Promise.allSettled([
      runClient.listRunEvents(runId),
      runClient.listRunLogs(runId),
      runClient.listProducedFiles(runId),
    ]);
    evidenceState = {
      events: evidenceResult(
        events,
        (value) => value.events,
        'Workflow run events could not be loaded.',
      ),
      logs: evidenceResult(
        logs,
        (value) => value.logs,
        'Workflow run logs could not be loaded.',
      ),
      producedFiles: evidenceResult(
        producedFiles,
        (value) => value.files,
        'Workflow run produced files could not be loaded.',
      ),
    };
  }

  async function downloadProducedFile(file: WorkflowRunProducedFileResource): Promise<void> {
    const run = currentRun();
    if (!run) {
      return;
    }
    downloadState = { status: 'running', label: `Preparing ${file.file_name}...` };
    try {
      const blob = await client().downloadProducedFileContent(run.id, file.file_id);
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = safeDownloadName(file.file_name);
      document.body.append(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      downloadState = { status: 'success', message: `${file.file_name} download started.` };
    } catch (error) {
      downloadState = {
        status: 'error',
        message: adminErrorMessage(error, 'Produced file download failed.'),
      };
    }
  }

  function currentRun(): WorkflowRunResource | null {
    return detailState.status === 'ready' ? detailState.run : null;
  }

  function evidenceResult<TResponse, TItem>(
    result: PromiseSettledResult<TResponse>,
    select: (response: TResponse) => readonly TItem[],
    fallback: string,
  ): WorkflowRunEvidenceState<TItem> {
    return result.status === 'fulfilled'
      ? { status: 'ready', items: select(result.value) }
      : { status: 'error', message: adminErrorMessage(result.reason, fallback) };
  }

  function emptyEvidenceState(): WorkflowRunDetailEvidenceState {
    return {
      events: { status: 'idle' } as WorkflowRunEvidenceState<WorkflowRunEventResource>,
      logs: { status: 'idle' } as WorkflowRunEvidenceState<WorkflowRunLogResource>,
      producedFiles: { status: 'idle' } as WorkflowRunEvidenceState<WorkflowRunProducedFileResource>,
    };
  }

  function safeDownloadName(fileName: string): string {
    return fileName.split(/[\\/]/).pop()?.trim() || 'workflow-output';
  }
</script>

<div class="mx-auto flex min-h-full w-full max-w-[1440px] flex-col gap-5 px-4 py-5 sm:px-6 lg:px-8" data-testid="workflow-run-detail-route">
  {#if detailState.status === 'loading'}
    <AdminMessage
      tone="loading"
      title="Loading workflow run"
      message={`Fetching ${detailState.runId} and its execution evidence.`}
      testId="workflow-run-detail-loading"
    />
  {:else if detailState.status === 'error'}
    <AdminMessage
      tone="error"
      title="Workflow run detail unavailable"
      message={detailState.message}
      testId="workflow-run-detail-error"
    />
  {:else}
    <WorkflowRunInspector
      run={detailState.run}
      evidence={evidenceState}
      {actionState}
      {downloadState}
      onRefresh={refreshRun}
      onCancel={cancelRun}
      onResume={resumeRun}
      onSubmitInput={submitInput}
      onReject={rejectRun}
      onDownloadProducedFile={downloadProducedFile}
    />
  {/if}
</div>
