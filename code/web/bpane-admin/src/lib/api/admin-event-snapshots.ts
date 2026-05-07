import { ControlSessionMapper } from './control-session-mapper';
import type { SessionResource } from './control-types';
import { expectBoolean, expectNumber, expectRecord, expectString } from './control-wire';

export type AdminSessionsSnapshotEvent = {
  readonly type: 'sessions.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly sessions: readonly SessionResource[];
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

export type AdminRecordingsSnapshot = {
  readonly sessionId: string;
  readonly recordingCount: number;
  readonly activeCount: number;
  readonly readyCount: number;
  readonly latestUpdatedAt: string | null;
};

export type AdminRecordingsSnapshotEvent = {
  readonly type: 'recordings.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly recordings: readonly AdminRecordingsSnapshot[];
};

export type AdminMcpDelegationSnapshot = {
  readonly sessionId: string;
  readonly delegatedClientId: string | null;
  readonly delegatedIssuer: string | null;
  readonly mcpOwner: boolean;
  readonly updatedAt: string;
};

export type AdminMcpDelegationSnapshotEvent = {
  readonly type: 'mcp_delegation.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly delegations: readonly AdminMcpDelegationSnapshot[];
};

export function toSessionsSnapshotEvent(object: Record<string, unknown>): AdminSessionsSnapshotEvent {
  const sessions = expectArray(object.sessions, 'sessions.snapshot event sessions');
  return {
    type: 'sessions.snapshot',
    sequence: expectNumber(object.sequence, 'sessions.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'sessions.snapshot event created_at'),
    sessions: sessions.map((session) => ControlSessionMapper.toSessionResource(session)),
  };
}

export function toWorkflowRunsSnapshotEvent(object: Record<string, unknown>): AdminWorkflowRunsSnapshotEvent {
  const workflowRuns = expectArray(object.workflow_runs, 'workflow_runs.snapshot event workflow_runs');
  return {
    type: 'workflow_runs.snapshot',
    sequence: expectNumber(object.sequence, 'workflow_runs.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'workflow_runs.snapshot event created_at'),
    workflowRuns: workflowRuns.map(toWorkflowRunSnapshot),
  };
}

export function toSessionFilesSnapshotEvent(object: Record<string, unknown>): AdminSessionFilesSnapshotEvent {
  const sessionFiles = expectArray(object.session_files, 'session_files.snapshot event session_files');
  return {
    type: 'session_files.snapshot',
    sequence: expectNumber(object.sequence, 'session_files.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'session_files.snapshot event created_at'),
    sessionFiles: sessionFiles.map(toSessionFilesSnapshot),
  };
}

export function toRecordingsSnapshotEvent(object: Record<string, unknown>): AdminRecordingsSnapshotEvent {
  const recordings = expectArray(object.recordings, 'recordings.snapshot event recordings');
  return {
    type: 'recordings.snapshot',
    sequence: expectNumber(object.sequence, 'recordings.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'recordings.snapshot event created_at'),
    recordings: recordings.map(toRecordingsSnapshot),
  };
}

export function toMcpDelegationSnapshotEvent(object: Record<string, unknown>): AdminMcpDelegationSnapshotEvent {
  const delegations = expectArray(object.mcp_delegations, 'mcp_delegation.snapshot event mcp_delegations');
  return {
    type: 'mcp_delegation.snapshot',
    sequence: expectNumber(object.sequence, 'mcp_delegation.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'mcp_delegation.snapshot event created_at'),
    delegations: delegations.map(toMcpDelegationSnapshot),
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

function toRecordingsSnapshot(payload: unknown): AdminRecordingsSnapshot {
  const object = expectRecord(payload, 'recordings snapshot');
  return {
    sessionId: expectString(object.session_id, 'recordings snapshot session_id'),
    recordingCount: expectNumber(object.recording_count, 'recordings snapshot recording_count'),
    activeCount: expectNumber(object.active_count, 'recordings snapshot active_count'),
    readyCount: expectNumber(object.ready_count, 'recordings snapshot ready_count'),
    latestUpdatedAt: object.latest_updated_at === null
      ? null
      : expectString(object.latest_updated_at, 'recordings snapshot latest_updated_at'),
  };
}

function toMcpDelegationSnapshot(payload: unknown): AdminMcpDelegationSnapshot {
  const object = expectRecord(payload, 'mcp delegation snapshot');
  return {
    sessionId: expectString(object.session_id, 'mcp delegation snapshot session_id'),
    delegatedClientId: object.delegated_client_id === null
      ? null
      : expectString(object.delegated_client_id, 'mcp delegation snapshot delegated_client_id'),
    delegatedIssuer: object.delegated_issuer === null
      ? null
      : expectString(object.delegated_issuer, 'mcp delegation snapshot delegated_issuer'),
    mcpOwner: expectBoolean(object.mcp_owner, 'mcp delegation snapshot mcp_owner'),
    updatedAt: expectString(object.updated_at, 'mcp delegation snapshot updated_at'),
  };
}

function expectArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value;
}
