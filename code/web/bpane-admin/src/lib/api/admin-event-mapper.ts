import { ControlSessionMapper } from './control-session-mapper';
import type { SessionResource } from './control-types';
import { expectNumber, expectRecord, expectString } from './control-wire';

export type AdminEventType =
  | 'sessions.snapshot'
  | 'workflow_runs.snapshot'
  | 'session_files.snapshot'
  | 'admin.error';

export type AdminSessionsSnapshotEvent = {
  readonly type: 'sessions.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly sessions: readonly SessionResource[];
};

export type AdminErrorEvent = {
  readonly type: 'admin.error';
  readonly sequence: number;
  readonly createdAt: string;
  readonly error: string;
};

export type AdminWorkflowRunSnapshot = {
  readonly id: string;
  readonly sessionId: string;
  readonly state: string;
  readonly updatedAt: string;
};

export type AdminWorkflowRunsSnapshotEvent = {
  readonly type: 'workflow_runs.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly workflowRuns: readonly AdminWorkflowRunSnapshot[];
};

export type AdminSessionFilesSnapshot = {
  readonly sessionId: string;
  readonly fileCount: number;
  readonly latestUpdatedAt: string | null;
};

export type AdminSessionFilesSnapshotEvent = {
  readonly type: 'session_files.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly sessionFiles: readonly AdminSessionFilesSnapshot[];
};

export type AdminEvent =
  | AdminSessionsSnapshotEvent
  | AdminWorkflowRunsSnapshotEvent
  | AdminSessionFilesSnapshotEvent
  | AdminErrorEvent;

export class AdminEventMapper {
  static toEvent(payload: unknown): AdminEvent {
    const object = expectRecord(payload, 'admin event');
    const eventType = expectString(object.event_type, 'admin event event_type');
    if (eventType === 'sessions.snapshot') {
      return toSessionsSnapshotEvent(object);
    }
    if (eventType === 'workflow_runs.snapshot') {
      return toWorkflowRunsSnapshotEvent(object);
    }
    if (eventType === 'session_files.snapshot') {
      return toSessionFilesSnapshotEvent(object);
    }
    if (eventType === 'admin.error') {
      return toAdminErrorEvent(object);
    }
    throw new Error(`unsupported admin event type: ${eventType}`);
  }
}

function toSessionsSnapshotEvent(object: Record<string, unknown>): AdminSessionsSnapshotEvent {
  const sessions = object.sessions;
  if (!Array.isArray(sessions)) {
    throw new Error('sessions.snapshot event sessions must be an array');
  }
  return {
    type: 'sessions.snapshot',
    sequence: expectNumber(object.sequence, 'sessions.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'sessions.snapshot event created_at'),
    sessions: sessions.map((session) => ControlSessionMapper.toSessionResource(session)),
  };
}

function toAdminErrorEvent(object: Record<string, unknown>): AdminErrorEvent {
  return {
    type: 'admin.error',
    sequence: expectNumber(object.sequence, 'admin.error event sequence'),
    createdAt: expectString(object.created_at, 'admin.error event created_at'),
    error: expectString(object.error, 'admin.error event error'),
  };
}

function toWorkflowRunsSnapshotEvent(object: Record<string, unknown>): AdminWorkflowRunsSnapshotEvent {
  const workflowRuns = object.workflow_runs;
  if (!Array.isArray(workflowRuns)) {
    throw new Error('workflow_runs.snapshot event workflow_runs must be an array');
  }
  return {
    type: 'workflow_runs.snapshot',
    sequence: expectNumber(object.sequence, 'workflow_runs.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'workflow_runs.snapshot event created_at'),
    workflowRuns: workflowRuns.map(toWorkflowRunSnapshot),
  };
}

function toWorkflowRunSnapshot(payload: unknown): AdminWorkflowRunSnapshot {
  const object = expectRecord(payload, 'workflow run snapshot');
  return {
    id: expectString(object.id, 'workflow run snapshot id'),
    sessionId: expectString(object.session_id, 'workflow run snapshot session_id'),
    state: expectString(object.state, 'workflow run snapshot state'),
    updatedAt: expectString(object.updated_at, 'workflow run snapshot updated_at'),
  };
}

function toSessionFilesSnapshotEvent(object: Record<string, unknown>): AdminSessionFilesSnapshotEvent {
  const sessionFiles = object.session_files;
  if (!Array.isArray(sessionFiles)) {
    throw new Error('session_files.snapshot event session_files must be an array');
  }
  return {
    type: 'session_files.snapshot',
    sequence: expectNumber(object.sequence, 'session_files.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'session_files.snapshot event created_at'),
    sessionFiles: sessionFiles.map(toSessionFilesSnapshot),
  };
}

function toSessionFilesSnapshot(payload: unknown): AdminSessionFilesSnapshot {
  const object = expectRecord(payload, 'session files snapshot');
  return {
    sessionId: expectString(object.session_id, 'session files snapshot session_id'),
    fileCount: expectNumber(object.file_count, 'session files snapshot file_count'),
    latestUpdatedAt: object.latest_updated_at === null
      ? null
      : expectString(object.latest_updated_at, 'session files snapshot latest_updated_at'),
  };
}
