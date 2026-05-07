import { expectNumber, expectRecord, expectString } from './control-wire';
import {
  type AdminMcpDelegationSnapshotEvent,
  type AdminRecordingsSnapshotEvent,
  type AdminSessionFilesSnapshotEvent,
  type AdminSessionsSnapshotEvent,
  type AdminWorkflowRunsSnapshotEvent,
  toMcpDelegationSnapshotEvent,
  toRecordingsSnapshotEvent,
  toSessionFilesSnapshotEvent,
  toSessionsSnapshotEvent,
  toWorkflowRunsSnapshotEvent,
} from './admin-event-snapshots';

export type AdminEventType =
  | 'sessions.snapshot'
  | 'workflow_runs.snapshot'
  | 'session_files.snapshot'
  | 'recordings.snapshot'
  | 'mcp_delegation.snapshot'
  | 'admin.error';

export type AdminErrorEvent = {
  readonly type: 'admin.error';
  readonly sequence: number;
  readonly createdAt: string;
  readonly error: string;
};

export type AdminEvent =
  | AdminSessionsSnapshotEvent
  | AdminWorkflowRunsSnapshotEvent
  | AdminSessionFilesSnapshotEvent
  | AdminRecordingsSnapshotEvent
  | AdminMcpDelegationSnapshotEvent
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
    if (eventType === 'recordings.snapshot') {
      return toRecordingsSnapshotEvent(object);
    }
    if (eventType === 'mcp_delegation.snapshot') {
      return toMcpDelegationSnapshotEvent(object);
    }
    if (eventType === 'admin.error') {
      return toAdminErrorEvent(object);
    }
    throw new Error(`unsupported admin event type: ${eventType}`);
  }
}

function toAdminErrorEvent(object: Record<string, unknown>): AdminErrorEvent {
  return {
    type: 'admin.error',
    sequence: expectNumber(object.sequence, 'admin.error event sequence'),
    createdAt: expectString(object.created_at, 'admin.error event created_at'),
    error: expectString(object.error, 'admin.error event error'),
  };
}
