import type {
  WorkflowRunEventListResponse,
  WorkflowRunEventResource,
  WorkflowRunInterventionRequestResource,
  WorkflowRunInterventionResource,
  WorkflowRunListResponse,
  WorkflowRunLogListResponse,
  WorkflowRunLogResource,
  WorkflowRunProducedFileListResponse,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
  WorkflowRunRuntimeResource,
} from './workflow-types';
import {
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  expectStringRecord,
  optionalString,
} from './control-wire';

export class WorkflowRunMapper {
  static toRunList(payload: unknown): WorkflowRunListResponse {
    const object = expectRecord(payload, 'workflow run list response');
    return {
      runs: expectArray(object.runs, 'workflow run list runs').map((entry) => this.toRun(entry)),
    };
  }

  static toRun(payload: unknown): WorkflowRunResource {
    const object = expectRecord(payload, 'workflow run resource');
    const sourceSystem = optionalString(object.source_system, 'workflow run source_system');
    const sourceReference = optionalString(object.source_reference, 'workflow run source_reference');
    const clientRequestId = optionalString(object.client_request_id, 'workflow run client_request_id');
    const error = optionalString(object.error, 'workflow run error');
    const startedAt = optionalString(object.started_at, 'workflow run started_at');
    const completedAt = optionalString(object.completed_at, 'workflow run completed_at');
    const runtime = object.runtime === undefined ? undefined : toRuntime(object.runtime);
    return {
      id: expectString(object.id, 'workflow run id'),
      workflow_definition_id: expectString(
        object.workflow_definition_id,
        'workflow run workflow_definition_id',
      ),
      workflow_definition_version_id: expectString(
        object.workflow_definition_version_id,
        'workflow run workflow_definition_version_id',
      ),
      workflow_version: expectString(object.workflow_version, 'workflow run workflow_version'),
      ...(sourceSystem !== undefined ? { source_system: sourceSystem } : {}),
      ...(sourceReference !== undefined ? { source_reference: sourceReference } : {}),
      ...(clientRequestId !== undefined ? { client_request_id: clientRequestId } : {}),
      state: expectString(object.state, 'workflow run state'),
      session_id: expectString(object.session_id, 'workflow run session_id'),
      automation_task_id: expectString(object.automation_task_id, 'workflow run automation_task_id'),
      ...(object.input !== undefined ? { input: object.input } : {}),
      ...(object.output !== undefined ? { output: object.output } : {}),
      ...(error !== undefined ? { error } : {}),
      artifact_refs: expectStringArray(object.artifact_refs ?? [], 'workflow run artifact_refs'),
      produced_files: expectArray(object.produced_files ?? [], 'workflow run produced_files')
        .map(toProducedFile),
      intervention: toIntervention(object.intervention),
      ...(runtime !== undefined ? { runtime } : {}),
      labels: expectStringRecord(object.labels ?? {}, 'workflow run labels'),
      ...(startedAt !== undefined ? { started_at: startedAt } : {}),
      ...(completedAt !== undefined ? { completed_at: completedAt } : {}),
      events_path: expectString(object.events_path, 'workflow run events_path'),
      logs_path: expectString(object.logs_path, 'workflow run logs_path'),
      created_at: expectString(object.created_at, 'workflow run created_at'),
      updated_at: expectString(object.updated_at, 'workflow run updated_at'),
    };
  }

  static toEventList(payload: unknown): WorkflowRunEventListResponse {
    const object = expectRecord(payload, 'workflow run event list response');
    return {
      events: expectArray(object.events, 'workflow run event list events').map(toEvent),
    };
  }

  static toLogList(payload: unknown): WorkflowRunLogListResponse {
    const object = expectRecord(payload, 'workflow run log list response');
    return {
      logs: expectArray(object.logs, 'workflow run log list logs').map(toLog),
    };
  }

  static toProducedFileList(payload: unknown): WorkflowRunProducedFileListResponse {
    const object = expectRecord(payload, 'workflow run produced file list response');
    return {
      files: expectArray(object.files, 'workflow run produced file list files').map(toProducedFile),
    };
  }
}

function toIntervention(value: unknown): WorkflowRunInterventionResource {
  const object = expectRecord(value, 'workflow run intervention');
  const pendingRequest = object.pending_request === undefined || object.pending_request === null
    ? object.pending_request
    : toInterventionRequest(object.pending_request);
  return {
    ...(pendingRequest !== undefined ? { pending_request: pendingRequest } : {}),
  };
}

function toInterventionRequest(value: unknown): WorkflowRunInterventionRequestResource {
  const object = expectRecord(value, 'workflow run intervention pending_request');
  const prompt = optionalString(object.prompt, 'workflow run intervention prompt');
  return {
    request_id: expectString(object.request_id, 'workflow run intervention request_id'),
    kind: expectString(object.kind, 'workflow run intervention kind'),
    ...(prompt !== undefined ? { prompt } : {}),
    ...(object.details !== undefined ? { details: object.details } : {}),
    requested_at: expectString(object.requested_at, 'workflow run intervention requested_at'),
  };
}

function toRuntime(value: unknown): WorkflowRunRuntimeResource | null {
  if (value === null) {
    return null;
  }
  const object = expectRecord(value, 'workflow run runtime');
  const holdUntil = optionalString(object.hold_until, 'workflow run runtime hold_until');
  const releasedAt = optionalString(object.released_at, 'workflow run runtime released_at');
  const releaseReason = optionalString(object.release_reason, 'workflow run runtime release_reason');
  const sessionState = optionalString(object.session_state, 'workflow run runtime session_state');
  return {
    resume_mode: expectString(object.resume_mode, 'workflow run runtime resume_mode'),
    exact_runtime_available: expectBoolean(
      object.exact_runtime_available,
      'workflow run runtime exact_runtime_available',
    ),
    ...(holdUntil !== undefined ? { hold_until: holdUntil } : {}),
    ...(releasedAt !== undefined ? { released_at: releasedAt } : {}),
    ...(releaseReason !== undefined ? { release_reason: releaseReason } : {}),
    ...(sessionState !== undefined ? { session_state: sessionState } : {}),
  };
}

function toProducedFile(value: unknown): WorkflowRunProducedFileResource {
  const object = expectRecord(value, 'workflow run produced file');
  const mediaType = optionalString(object.media_type, 'workflow run produced file media_type');
  return {
    workspace_id: expectString(object.workspace_id, 'workflow run produced file workspace_id'),
    file_id: expectString(object.file_id, 'workflow run produced file file_id'),
    file_name: expectString(object.file_name, 'workflow run produced file file_name'),
    ...(mediaType !== undefined ? { media_type: mediaType } : {}),
    byte_count: expectNumber(object.byte_count, 'workflow run produced file byte_count'),
    sha256_hex: expectString(object.sha256_hex, 'workflow run produced file sha256_hex'),
    ...(object.provenance !== undefined ? { provenance: object.provenance } : {}),
    content_path: expectString(object.content_path, 'workflow run produced file content_path'),
    created_at: expectString(object.created_at, 'workflow run produced file created_at'),
  };
}

function toEvent(value: unknown): WorkflowRunEventResource {
  const object = expectRecord(value, 'workflow run event');
  const automationTaskId = optionalString(
    object.automation_task_id,
    'workflow run event automation_task_id',
  );
  return {
    id: expectString(object.id, 'workflow run event id'),
    run_id: expectString(object.run_id, 'workflow run event run_id'),
    event_type: expectString(object.event_type, 'workflow run event event_type'),
    message: expectString(object.message, 'workflow run event message'),
    source: expectString(object.source, 'workflow run event source'),
    ...(automationTaskId !== undefined ? { automation_task_id: automationTaskId } : {}),
    ...(object.data !== undefined ? { data: object.data } : {}),
    created_at: expectString(object.created_at, 'workflow run event created_at'),
  };
}

function toLog(value: unknown): WorkflowRunLogResource {
  const object = expectRecord(value, 'workflow run log');
  const automationTaskId = optionalString(
    object.automation_task_id,
    'workflow run log automation_task_id',
  );
  return {
    id: expectString(object.id, 'workflow run log id'),
    run_id: expectString(object.run_id, 'workflow run log run_id'),
    stream: expectString(object.stream, 'workflow run log stream'),
    source: expectString(object.source, 'workflow run log source'),
    ...(automationTaskId !== undefined ? { automation_task_id: automationTaskId } : {}),
    message: expectString(object.message, 'workflow run log message'),
    created_at: expectString(object.created_at, 'workflow run log created_at'),
  };
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value;
}

function expectStringArray(value: unknown, label: string): readonly string[] {
  return expectArray(value, label).map((entry) => expectString(entry, `${label} entry`));
}
