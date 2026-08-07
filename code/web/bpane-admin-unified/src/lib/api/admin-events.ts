import {
  AdminEventMapper as SharedAdminEventMapper,
  type AdminErrorEvent,
  type AdminEvent as SharedAdminEvent,
  type AdminEventType,
  type AdminMcpDelegationSnapshot,
  type AdminMcpDelegationSnapshotEvent,
  type AdminRecordingsSnapshot,
  type AdminRecordingsSnapshotEvent,
  type AdminSessionFilesSnapshot,
  type AdminSessionFilesSnapshotEvent,
  type AdminSessionsSnapshotEvent as SharedAdminSessionsSnapshotEvent,
  type AdminWorkflowRunSnapshot,
  type AdminWorkflowRunsSnapshotEvent,
} from '@browserpane/admin-auth';
import { toSessionResource } from '$lib/sessions/session-client';
import type { SessionResource } from '$lib/sessions/session-types';

export type {
  AdminErrorEvent,
  AdminEventType,
  AdminMcpDelegationSnapshot,
  AdminMcpDelegationSnapshotEvent,
  AdminRecordingsSnapshot,
  AdminRecordingsSnapshotEvent,
  AdminSessionFilesSnapshot,
  AdminSessionFilesSnapshotEvent,
  AdminWorkflowRunSnapshot,
  AdminWorkflowRunsSnapshotEvent,
};
export type AdminSessionsSnapshotEvent = SharedAdminSessionsSnapshotEvent<SessionResource>;
export type AdminEvent = SharedAdminEvent<SessionResource>;

const mapper = new SharedAdminEventMapper<SessionResource>(toSessionResource);

export function toAdminEvent(payload: unknown): AdminEvent {
  return mapper.toEvent(payload);
}
