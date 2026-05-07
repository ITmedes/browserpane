import type { SessionResource } from '../api/control-types';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
  WorkflowRunEventResource,
  WorkflowRunLogResource,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
} from '../api/workflow-types';

export type WorkflowDefinitionOption = {
  readonly id: string;
  readonly label: string;
  readonly latestVersion: string;
};

export type WorkflowOperationsViewModel = {
  readonly title: string;
  readonly status: string;
  readonly note: string;
  readonly selectedSessionLabel: string;
  readonly definitionOptions: readonly WorkflowDefinitionOption[];
  readonly selectedWorkflowId: string;
  readonly selectedVersion: string;
  readonly executorLabel: string;
  readonly currentRunId: string;
  readonly currentRunState: string;
  readonly pendingPrompt: string;
  readonly recentLogs: readonly WorkflowRunTextRow[];
  readonly recentEvents: readonly WorkflowRunTextRow[];
  readonly producedFiles: readonly WorkflowProducedFileRow[];
  readonly logCount: number;
  readonly eventCount: number;
  readonly fileCount: number;
  readonly error: string | null;
  readonly loading: boolean;
  readonly canRefresh: boolean;
  readonly canRun: boolean;
  readonly canRefreshRun: boolean;
  readonly canCancel: boolean;
  readonly canReleaseHold: boolean;
  readonly canSubmitInput: boolean;
};

export type WorkflowRunTextRow = {
  readonly id: string;
  readonly label: string;
  readonly message: string;
};

export type WorkflowProducedFileRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
};

export class WorkflowOperationsViewModelBuilder {
  static build(input: {
    readonly selectedSession: SessionResource | null;
    readonly definitions: readonly WorkflowDefinitionResource[];
    readonly selectedWorkflowId: string;
    readonly selectedVersion: string;
    readonly selectedVersionResource: WorkflowDefinitionVersionResource | null;
    readonly currentRun: WorkflowRunResource | null;
    readonly logs: readonly WorkflowRunLogResource[];
    readonly events: readonly WorkflowRunEventResource[];
    readonly files: readonly WorkflowRunProducedFileResource[];
    readonly loading: boolean;
    readonly actionInFlight: boolean;
    readonly error: string | null;
    readonly inputValid: boolean;
    readonly interventionInputValid: boolean;
  }): WorkflowOperationsViewModel {
    const hasSession = Boolean(input.selectedSession);
    const hasDefinition = Boolean(input.selectedWorkflowId && input.selectedVersion);
    const terminal = isTerminal(input.currentRun?.state ?? null);
    const pendingRequest = input.currentRun?.intervention.pending_request ?? null;
    return {
      title: hasSession ? 'Workflow run controls' : 'Select a session for workflow runs',
      status: statusLabel(input.loading, input.error, input.currentRun),
      note: note(input.definitions.length, hasSession, input.error),
      selectedSessionLabel: input.selectedSession?.id ?? '--',
      definitionOptions: input.definitions.map(toDefinitionOption),
      selectedWorkflowId: input.selectedWorkflowId,
      selectedVersion: input.selectedVersion,
      executorLabel: input.selectedVersionResource?.executor ?? 'definition not loaded',
      currentRunId: input.currentRun?.id ?? '--',
      currentRunState: input.currentRun?.state ?? 'not started',
      pendingPrompt: pendingRequest?.prompt ?? pendingRequest?.kind ?? 'No pending operator input.',
      recentLogs: input.logs.slice(-5).reverse().map(toLogRow),
      recentEvents: input.events.slice(-5).reverse().map(toEventRow),
      producedFiles: input.files.map(toFileRow),
      logCount: input.logs.length,
      eventCount: input.events.length,
      fileCount: input.files.length,
      error: input.error,
      loading: input.loading || input.actionInFlight,
      canRefresh: !input.loading && !input.actionInFlight,
      canRun: hasSession && hasDefinition && input.inputValid && !input.actionInFlight,
      canRefreshRun: Boolean(input.currentRun) && !input.actionInFlight,
      canCancel: Boolean(input.currentRun) && !terminal && !input.actionInFlight,
      canReleaseHold: Boolean(pendingRequest) && !input.actionInFlight,
      canSubmitInput: Boolean(pendingRequest) && input.interventionInputValid && !input.actionInFlight,
    };
  }
}

function toDefinitionOption(definition: WorkflowDefinitionResource): WorkflowDefinitionOption {
  return {
    id: definition.id,
    label: `${definition.name}${definition.latest_version ? ` (${definition.latest_version})` : ''}`,
    latestVersion: definition.latest_version ?? '',
  };
}

function statusLabel(
  loading: boolean,
  error: string | null,
  currentRun: WorkflowRunResource | null,
): string {
  if (loading) {
    return 'loading';
  }
  if (error) {
    return 'attention';
  }
  return currentRun?.state ?? 'ready';
}

function note(definitionCount: number, hasSession: boolean, error: string | null): string {
  if (error) {
    return error;
  }
  if (!hasSession) {
    return 'Select or create a session before invoking a workflow.';
  }
  if (definitionCount === 0) {
    return 'No workflow definitions are available for this owner yet.';
  }
  return 'Invoke a workflow against the selected session and inspect run output.';
}

function isTerminal(state: string | null): boolean {
  return state === 'succeeded' || state === 'failed' || state === 'cancelled';
}

function toLogRow(log: WorkflowRunLogResource): WorkflowRunTextRow {
  return {
    id: log.id,
    label: `${log.source} ${log.stream}`,
    message: log.message,
  };
}

function toEventRow(event: WorkflowRunEventResource): WorkflowRunTextRow {
  return {
    id: event.id,
    label: event.event_type,
    message: event.message,
  };
}

function toFileRow(file: WorkflowRunProducedFileResource): WorkflowProducedFileRow {
  return {
    id: file.file_id,
    name: file.file_name,
    description: `${formatBytes(file.byte_count)}${file.media_type ? ` · ${file.media_type}` : ''}`,
  };
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  return `${Math.round(bytes / 102.4) / 10} KiB`;
}
