import type { WorkflowClient } from '../api/workflow-client';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
  WorkflowRunEventResource,
  WorkflowRunLogResource,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
} from '../api/workflow-types';

export type WorkflowDefinitionSelection = {
  readonly definitions: readonly WorkflowDefinitionResource[];
  readonly selectedWorkflowId: string;
  readonly selectedVersion: string;
  readonly selectedVersionResource: WorkflowDefinitionVersionResource | null;
};

export type WorkflowRunSnapshot = {
  readonly run: WorkflowRunResource;
  readonly events: readonly WorkflowRunEventResource[];
  readonly logs: readonly WorkflowRunLogResource[];
  readonly files: readonly WorkflowRunProducedFileResource[];
};

export class WorkflowOperationsService {
  constructor(private readonly workflowClient: WorkflowClient) {}

  async loadDefinitions(
    selectedWorkflowId: string,
    selectedVersion: string,
  ): Promise<WorkflowDefinitionSelection> {
    const definitions = (await this.workflowClient.listDefinitions()).workflows;
    const selected = definitions.find((entry) => entry.id === selectedWorkflowId)
      ?? definitions[0]
      ?? null;
    const workflowId = selected?.id ?? '';
    const version = selected?.latest_version ?? selectedVersion;
    return {
      definitions,
      selectedWorkflowId: workflowId,
      selectedVersion: version,
      selectedVersionResource: await this.loadVersionOrNull(workflowId, version),
    };
  }

  async loadVersionOrNull(
    workflowId: string,
    version: string,
  ): Promise<WorkflowDefinitionVersionResource | null> {
    if (!workflowId || !version) {
      return null;
    }
    return await this.workflowClient.getDefinitionVersion(workflowId, version);
  }

  async invokeRun(input: {
    readonly sessionId: string;
    readonly workflowId: string;
    readonly version: string;
    readonly runInput: unknown;
  }): Promise<WorkflowRunResource> {
    return await this.workflowClient.createRun({
      workflow_id: input.workflowId,
      version: input.version,
      session: { existing_session_id: input.sessionId },
      input: input.runInput,
      client_request_id: `bpane-admin-${globalThis.crypto?.randomUUID?.() ?? Date.now()}`,
      labels: { source: 'bpane-admin' },
    });
  }

  async refreshRun(runId: string): Promise<WorkflowRunSnapshot> {
    const [run, events, logs, files] = await Promise.all([
      this.workflowClient.getRun(runId),
      this.workflowClient.listRunEvents(runId),
      this.workflowClient.listRunLogs(runId),
      this.workflowClient.listProducedFiles(runId),
    ]);
    return {
      run,
      events: events.events,
      logs: logs.logs,
      files: files.files,
    };
  }

  async cancelRun(runId: string): Promise<WorkflowRunResource> {
    return await this.workflowClient.cancelRun(runId);
  }

  async releaseHold(runId: string): Promise<WorkflowRunResource> {
    return await this.workflowClient.resumeRun(runId, { comment: 'released from admin' });
  }

  async submitInput(runId: string, input: unknown): Promise<WorkflowRunResource> {
    return await this.workflowClient.submitRunInput(runId, {
      input,
      comment: 'operator input from admin',
    });
  }

  async downloadFile(file: WorkflowRunProducedFileResource): Promise<Blob> {
    return await this.workflowClient.downloadProducedFileContent(file);
  }
}
