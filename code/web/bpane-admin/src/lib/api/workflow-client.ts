import {
  sendAuthenticatedRequest,
  type AccessTokenProvider,
  type AuthenticationFailureHandler,
  type FetchLike,
} from './authenticated-api';
import { WorkflowMapper } from './workflow-mapper';
import { WorkflowRunMapper } from './workflow-run-mapper';
import type {
  CreateWorkflowRunCommand,
  RejectWorkflowRunCommand,
  ResumeWorkflowRunCommand,
  SubmitWorkflowRunInputCommand,
  WorkflowDefinitionListResponse,
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
  WorkflowRunEventListResponse,
  WorkflowRunLogListResponse,
  WorkflowRunProducedFileListResponse,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
} from './workflow-types';

export type WorkflowClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly onAuthenticationFailure?: AuthenticationFailureHandler;
  readonly fetchImpl?: FetchLike;
};

export class WorkflowClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #onAuthenticationFailure: AuthenticationFailureHandler | undefined;
  readonly #fetchImpl: FetchLike;

  constructor(options: WorkflowClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
    this.#fetchImpl = options.fetchImpl ?? fetch;
  }

  async listDefinitions(): Promise<WorkflowDefinitionListResponse> {
    const payload = await this.#request('GET', '/api/v1/workflows');
    return WorkflowMapper.toDefinitionList(payload);
  }

  async getDefinition(workflowId: string): Promise<WorkflowDefinitionResource> {
    const payload = await this.#request('GET', `/api/v1/workflows/${encodeURIComponent(workflowId)}`);
    return WorkflowMapper.toDefinition(payload);
  }

  async getDefinitionVersion(
    workflowId: string,
    version: string,
  ): Promise<WorkflowDefinitionVersionResource> {
    const payload = await this.#request(
      'GET',
      `/api/v1/workflows/${encodeURIComponent(workflowId)}/versions/${encodeURIComponent(version)}`,
    );
    return WorkflowMapper.toDefinitionVersion(payload);
  }

  async createRun(command: CreateWorkflowRunCommand): Promise<WorkflowRunResource> {
    const payload = await this.#request('POST', '/api/v1/workflow-runs', command);
    return WorkflowRunMapper.toRun(payload);
  }

  async getRun(runId: string): Promise<WorkflowRunResource> {
    const payload = await this.#request('GET', `/api/v1/workflow-runs/${encodeURIComponent(runId)}`);
    return WorkflowRunMapper.toRun(payload);
  }

  async cancelRun(runId: string): Promise<WorkflowRunResource> {
    const payload = await this.#request(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/cancel`,
    );
    return WorkflowRunMapper.toRun(payload);
  }

  async resumeRun(
    runId: string,
    command: ResumeWorkflowRunCommand = {},
  ): Promise<WorkflowRunResource> {
    const payload = await this.#request(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/resume`,
      command,
    );
    return WorkflowRunMapper.toRun(payload);
  }

  async submitRunInput(
    runId: string,
    command: SubmitWorkflowRunInputCommand,
  ): Promise<WorkflowRunResource> {
    const payload = await this.#request(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/submit-input`,
      command,
    );
    return WorkflowRunMapper.toRun(payload);
  }

  async rejectRun(runId: string, command: RejectWorkflowRunCommand): Promise<WorkflowRunResource> {
    const payload = await this.#request(
      'POST',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/reject`,
      command,
    );
    return WorkflowRunMapper.toRun(payload);
  }

  async listRunEvents(runId: string): Promise<WorkflowRunEventListResponse> {
    const payload = await this.#request('GET', `/api/v1/workflow-runs/${encodeURIComponent(runId)}/events`);
    return WorkflowRunMapper.toEventList(payload);
  }

  async listRunLogs(runId: string): Promise<WorkflowRunLogListResponse> {
    const payload = await this.#request('GET', `/api/v1/workflow-runs/${encodeURIComponent(runId)}/logs`);
    return WorkflowRunMapper.toLogList(payload);
  }

  async listProducedFiles(runId: string): Promise<WorkflowRunProducedFileListResponse> {
    const payload = await this.#request(
      'GET',
      `/api/v1/workflow-runs/${encodeURIComponent(runId)}/produced-files`,
    );
    return WorkflowRunMapper.toProducedFileList(payload);
  }

  async downloadProducedFileContent(file: WorkflowRunProducedFileResource): Promise<Blob> {
    const response = await this.#send('GET', file.content_path, undefined, '*/*');
    return await response.blob();
  }

  async #request(method: string, path: string, body?: unknown): Promise<unknown> {
    const response = await this.#send(method, path, body, 'application/json');
    return await response.json();
  }

  async #send(method: string, path: string, body?: unknown, accept = 'application/json'): Promise<Response> {
    return await sendAuthenticatedRequest({
      baseUrl: this.#baseUrl,
      accessTokenProvider: this.#accessTokenProvider,
      fetchImpl: this.#fetchImpl,
      onAuthenticationFailure: this.#onAuthenticationFailure,
      method,
      path,
      body,
      accept,
    });
  }
}
