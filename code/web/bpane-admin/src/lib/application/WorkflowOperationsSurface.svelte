<script lang="ts">
  import { onMount } from 'svelte';
  import type { SessionResource } from '../api/control-types';
  import type { WorkflowClient } from '../api/workflow-client';
  import type { WorkflowDefinitionResource, WorkflowDefinitionVersionResource, WorkflowRunEventResource, WorkflowRunLogResource, WorkflowRunProducedFileResource, WorkflowRunResource } from '../api/workflow-types';
  import type { AdminMessageFeedback } from '../presentation/admin-message-types';
  import WorkflowOperationsPanel from '../presentation/WorkflowOperationsPanel.svelte';
  import { WorkflowOperationsViewModelBuilder } from '../presentation/workflow-operations-view-model';
  import { WorkflowOperationsService } from './workflow-operations-service';

  type WorkflowOperationsSurfaceProps = {
    readonly workflowClient: WorkflowClient;
    readonly selectedSession: SessionResource | null;
    readonly connected: boolean;
    readonly onCreateSession: () => void;
    readonly onConnectSession: () => void;
  };

  let {
    workflowClient,
    selectedSession,
    connected,
    onCreateSession,
    onConnectSession,
  }: WorkflowOperationsSurfaceProps = $props();
  const workflowService = $derived(new WorkflowOperationsService(workflowClient));
  let definitions = $state<readonly WorkflowDefinitionResource[]>([]);
  let selectedWorkflowId = $state('');
  let selectedVersion = $state('');
  let selectedVersionResource = $state<WorkflowDefinitionVersionResource | null>(null);
  let currentRun = $state<WorkflowRunResource | null>(null);
  let events = $state<readonly WorkflowRunEventResource[]>([]);
  let logs = $state<readonly WorkflowRunLogResource[]>([]);
  let files = $state<readonly WorkflowRunProducedFileResource[]>([]);
  let inputText = $state('{}');
  let interventionInputText = $state('{}');
  let loading = $state(false);
  let actionInFlight = $state(false);
  let error = $state<string | null>(null);
  let feedback = $state<AdminMessageFeedback | null>(null);
  let currentSessionId = $state<string | null>(null);

  const inputValid = $derived(isJson(inputText));
  const interventionInputValid = $derived(isJson(interventionInputText));
  const viewModel = $derived(WorkflowOperationsViewModelBuilder.build({
    selectedSession, definitions, selectedWorkflowId, selectedVersion, selectedVersionResource,
    currentRun, logs, events, files, loading, actionInFlight, error, inputValid, connected,
    interventionInputValid,
  }));

  onMount(() => {
    void loadDefinitions(false);
  });

  $effect(() => {
    const nextSessionId = selectedSession?.id ?? null;
    if (nextSessionId === currentSessionId) {
      return;
    }
    currentSessionId = nextSessionId;
    currentRun = null;
    events = [];
    logs = [];
    files = [];
    error = null;
    feedback = null;
  });

  async function loadDefinitions(showFeedback = true): Promise<void> {
    loading = true;
    error = null;
    feedback = null;
    try {
      const selection = await workflowService.loadDefinitions(selectedWorkflowId, selectedVersion);
      definitions = selection.definitions;
      selectedWorkflowId = selection.selectedWorkflowId;
      selectedVersion = selection.selectedVersion;
      selectedVersionResource = selection.selectedVersionResource;
      if (showFeedback) {
        feedback = successFeedback(`${selection.definitions.length} workflow template${selection.definitions.length === 1 ? '' : 's'} refreshed.`);
      }
    } catch (loadError) {
      error = errorMessage(loadError);
      feedback = null;
    } finally {
      loading = false;
    }
  }

  async function selectWorkflow(workflowId: string): Promise<void> {
    error = null;
    feedback = null;
    selectedWorkflowId = workflowId;
    selectedVersion = definitions.find((entry) => entry.id === workflowId)?.latest_version ?? '';
    selectedVersionResource = null;
    await loadSelectedVersion();
  }

  function updateSelectedVersion(version: string): void {
    selectedVersion = version.trim();
    selectedVersionResource = null;
    error = null;
    feedback = null;
  }

  async function loadSelectedVersion(): Promise<void> {
    selectedVersionResource = null;
    if (!selectedWorkflowId || !selectedVersion) {
      return;
    }
    await runAction(async () => {
      selectedVersionResource = await workflowService.loadVersionOrNull(
        selectedWorkflowId,
        selectedVersion,
      );
    });
  }

  async function invokeRun(): Promise<void> {
    if (viewModel.invokeBlockedReason) {
      error = viewModel.invokeBlockedReason;
      feedback = null;
      return;
    }
    if (!selectedSession) {
      error = 'Select or create a session before invoking a workflow. The selected session is the workflow baseline.';
      feedback = null;
      return;
    }
    await runAction(async () => {
      if (!selectedVersionResource) {
        selectedVersionResource = await workflowService.loadVersionOrNull(
          selectedWorkflowId,
          selectedVersion,
        );
      }
      currentRun = await workflowService.invokeRun({
        sessionId: selectedSession.id,
        workflowId: selectedWorkflowId,
        version: selectedVersion,
        runInput: parseJson(inputText),
      });
      await refreshRunResources(false);
    }, () => currentRun ? `Workflow run ${shortId(currentRun.id)} was invoked.` : 'Workflow run was invoked.');
  }

  async function refreshRunResources(withBusy = true): Promise<void> {
    if (!currentRun) {
      return;
    }
    const refresh = async (): Promise<void> => {
      const snapshot = await workflowService.refreshRun(currentRun!.id);
      currentRun = snapshot.run;
      events = snapshot.events;
      logs = snapshot.logs;
      files = snapshot.files;
    };
    if (withBusy) {
      await runAction(refresh, () => currentRun ? `Workflow run ${shortId(currentRun.id)} refreshed.` : 'Workflow run refreshed.');
    } else {
      await refresh();
    }
  }

  async function mutateRun(
    successMessage: (run: WorkflowRunResource) => string,
    action: () => Promise<WorkflowRunResource>,
  ): Promise<void> {
    await runAction(async () => {
      currentRun = await action();
      await refreshRunResources(false);
    }, () => currentRun ? successMessage(currentRun) : 'Workflow run updated.');
  }

  async function runAction(
    action: () => Promise<void>,
    successMessage?: () => string,
  ): Promise<void> {
    actionInFlight = true;
    error = null;
    feedback = null;
    try {
      await action();
      if (successMessage) {
        feedback = successFeedback(successMessage());
      }
    } catch (actionError) {
      error = errorMessage(actionError);
      feedback = null;
    } finally {
      actionInFlight = false;
    }
  }

  async function downloadProducedFile(fileId: string): Promise<void> {
    const file = files.find((entry) => entry.file_id === fileId);
    if (!file) {
      return;
    }
    error = null;
    feedback = null;
    try {
      const url = URL.createObjectURL(await workflowService.downloadFile(file));
      const link = document.createElement('a');
      link.href = url;
      link.download = file.file_name;
      document.body.append(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      feedback = successFeedback(`Produced file ${file.file_name} download started.`);
    } catch (downloadError) {
      error = errorMessage(downloadError);
      feedback = null;
    }
  }

  function parseJson(text: string): unknown {
    return JSON.parse(text.trim() || '{}');
  }

  function isJson(text: string): boolean {
    try {
      parseJson(text);
      return true;
    } catch {
      return false;
    }
  }

  function errorMessage(value: unknown): string {
    return value instanceof Error ? value.message : 'Unexpected workflow operation error';
  }

  function successFeedback(message: string): AdminMessageFeedback {
    return { variant: 'success', title: 'Workflow updated', message, testId: 'workflow-message' };
  }

  function shortId(value: string): string {
    return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
  }
</script>

<WorkflowOperationsPanel
  {viewModel} {inputText} {interventionInputText} {feedback}
  onRefreshDefinitions={() => void loadDefinitions()}
  onWorkflowChange={(workflowId) => void selectWorkflow(workflowId)}
  onVersionChange={updateSelectedVersion}
  onInputTextChange={(next) => { inputText = next; }}
  onInterventionInputChange={(next) => { interventionInputText = next; }}
  onCreateSession={onCreateSession}
  onConnectSession={onConnectSession}
  onInvokeRun={() => void invokeRun()}
  onRefreshRun={() => void refreshRunResources()}
  onCancelRun={() => currentRun && void mutateRun((run) => `Workflow run ${shortId(run.id)} was cancelled.`, () => workflowService.cancelRun(currentRun!.id))}
  onReleaseHold={() => currentRun && void mutateRun((run) => `Workflow run ${shortId(run.id)} hold was released.`, () => workflowService.releaseHold(currentRun!.id))}
  onSubmitInput={() => currentRun && void mutateRun((run) => `Operator input was submitted for workflow run ${shortId(run.id)}.`, () => workflowService.submitInput(currentRun!.id, parseJson(interventionInputText)))}
  onDownloadProducedFile={(fileId) => void downloadProducedFile(fileId)}
/>
