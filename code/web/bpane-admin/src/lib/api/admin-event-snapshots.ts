import type {
  AdminMcpDelegationSnapshot as SharedAdminMcpDelegationSnapshot,
  AdminMcpDelegationSnapshotEvent as SharedAdminMcpDelegationSnapshotEvent,
  AdminRecordingsSnapshot as SharedAdminRecordingsSnapshot,
  AdminRecordingsSnapshotEvent as SharedAdminRecordingsSnapshotEvent,
  AdminSessionFilesSnapshot as SharedAdminSessionFilesSnapshot,
  AdminSessionFilesSnapshotEvent as SharedAdminSessionFilesSnapshotEvent,
  AdminSessionsSnapshotEvent as SharedAdminSessionsSnapshotEvent,
  AdminWorkflowRunSnapshot as SharedAdminWorkflowRunSnapshot,
  AdminWorkflowRunsSnapshotEvent as SharedAdminWorkflowRunsSnapshotEvent,
} from '@browserpane/admin-auth';
import type { SessionResource } from './control-types';

export type AdminSessionsSnapshotEvent = SharedAdminSessionsSnapshotEvent<SessionResource>;
export type AdminWorkflowRunSnapshot = SharedAdminWorkflowRunSnapshot;
export type AdminWorkflowRunsSnapshotEvent = SharedAdminWorkflowRunsSnapshotEvent;
export type AdminSessionFilesSnapshot = SharedAdminSessionFilesSnapshot;
export type AdminSessionFilesSnapshotEvent = SharedAdminSessionFilesSnapshotEvent;
export type AdminRecordingsSnapshot = SharedAdminRecordingsSnapshot;
export type AdminRecordingsSnapshotEvent = SharedAdminRecordingsSnapshotEvent;
export type AdminMcpDelegationSnapshot = SharedAdminMcpDelegationSnapshot;
export type AdminMcpDelegationSnapshotEvent = SharedAdminMcpDelegationSnapshotEvent;
