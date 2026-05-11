<script lang="ts">
  import { onMount } from 'svelte';
  import type { SessionResource } from '../api/control-types';
  import type { WorkflowClient } from '../api/workflow-client';
  import type { WorkflowDefinitionResource, WorkflowDefinitionVersionResource, WorkflowRunEventResource, WorkflowRunLogResource, WorkflowRunProducedFileResource, WorkflowRunResource } from '../api/workflow-types';
  import WorkflowOperationsPanel from '../presentation/WorkflowOperationsPanel.svelte';
  import { WorkflowOperationsViewModelBuilder } from '../presentation/workflow-operations-view-model';
  import { WorkflowOperationsService } from './workflow-operations-service';

  type WorkflowOperationsSurfaceProps = {
    readonly workflowClient: WorkflowClient;
    readonly selectedSession: SessionResource | null;
  };

  let { workflowClient, selectedSession }: WorkflowOperationsSurfaceProps = $props();
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
  let currentSessionId = $state<string | null>(null);

  const inputValid = $derived(isJson(inputText));
  const interventionInputValid = $derived(isJson(interventionInputText));
  const viewModel = $derived(WorkflowOperationsViewModelBuilder.build({
    selectedSession, definitions, selectedWorkflowId, selectedVersion, selectedVersionResource,
    currentRun, logs, events, files, loading, actionInFlight, error, inputValid,
    interventionInputValid,
  }));

  onMount(() => {
    void loadDefinitions();
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
  });

  async function loadDefinitions(): Promise<void> {
    loading = true;
    error = null;
    try {
      const selection = await workflowService.loadDefinitions(selectedWorkflowId, selectedVersion);
      definitions = selection.definitions;
      selectedWorkflowId = selection.selectedWorkflowId;
      selectedVersion = selection.selectedVersion;
      selectedVersionResource = selection.selectedVersionResource;
    } catch (loadError) {
      error = errorMessage(loadError);
    } finally {
      loading = false;
    }
  }

  async function selectWorkflow(workflowId: string): Promise<void> {
    selectedWorkflowId = workflowId;
    selectedVersion = definitions.find((entry) => entry.id === workflowId)?.latest_version ?? '';
    await loadSelectedVersion();
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
    if (!selectedSession || !selectedWorkflowId || !selectedVersion) {
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
    });
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
      await runAction(refresh);
    } else {
      await refresh();
    }
  }

  async function mutateRun(action: () => Promise<WorkflowRunResource>): Promise<void> {
    await runAction(async () => {
      currentRun = await action();
      await refreshRunResources(false);
    });
  }

  async function runAction(action: () => Promise<void>): Promise<void> {
    actionInFlight = true;
    error = null;
    try {
      await action();
    } catch (actionError) {
      error = errorMessage(actionError);
    } finally {
      actionInFlight = false;
    }
  }

  async function downloadProducedFile(fileId: string): Promise<void> {
    const file = files.find((entry) => entry.file_id === fileId);
    if (!file) {
      return;
    }
    try {
      const url = URL.createObjectURL(await workflowService.downloadFile(file));
      const link = document.createElement('a');
      link.href = url;
      link.download = file.file_name;
      document.body.append(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
    } catch (downloadError) {
      error = errorMessage(downloadError);
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
</script>

<WorkflowOperationsPanel
  {viewModel} {inputText} {interventionInputText}
  onRefreshDefinitions={() => void loadDefinitions()}
  onWorkflowChange={(workflowId) => void selectWorkflow(workflowId)}
  onVersionChange={(version) => { selectedVersion = version.trim(); selectedVersionResource = null; }}
  onInputTextChange={(next) => { inputText = next; }}
  onInterventionInputChange={(next) => { interventionInputText = next; }}
  onInvokeRun={() => void invokeRun()}
  onRefreshRun={() => void refreshRunResources()}
  onCancelRun={() => currentRun && void mutateRun(() => workflowService.cancelRun(currentRun!.id))}
  onReleaseHold={() => currentRun && void mutateRun(() => workflowService.releaseHold(currentRun!.id))}
  onSubmitInput={() => currentRun && void mutateRun(() => workflowService.submitInput(currentRun!.id, parseJson(interventionInputText)))}
  onDownloadProducedFile={(fileId) => void downloadProducedFile(fileId)}
/>
