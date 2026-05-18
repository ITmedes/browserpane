import type {
  AdminEventClient,
  AdminEventConnectionStatus,
  AdminEventSubscription,
} from '../api/admin-event-client';
import type {
  AdminMcpDelegationSnapshot,
  AdminRecordingsSnapshot,
  AdminSessionFilesSnapshot,
  AdminWorkflowRunSnapshot,
} from '../api/admin-event-snapshots';
import type { AdminEvent } from '../api/admin-event-mapper';
import type { SessionResource } from '../api/control-types';
import type { AdminLogEntry } from '../presentation/logs-view-model';
import { AdminLogEntryFactory } from './admin-log-entries';

type AdminSessionEventSyncHandlers = {
  readonly onSessions: (sessions: readonly SessionResource[]) => void;
  readonly onLoadingChange: (loading: boolean) => void;
  readonly onError: (error: string | null) => void;
  readonly onLog: (entry: AdminLogEntry) => void;
  readonly onConnectionStatus?: (status: AdminEventConnectionStatus) => void;
  readonly onSessionFilesSnapshot?: (sessionFiles: readonly AdminSessionFilesSnapshot[]) => void;
  readonly onRecordingsSnapshot?: (recordings: readonly AdminRecordingsSnapshot[]) => void;
  readonly onMcpDelegationSnapshot?: (delegations: readonly AdminMcpDelegationSnapshot[]) => void;
  readonly onWorkflowRunsSnapshot?: (runs: readonly AdminWorkflowRunSnapshot[]) => void;
};

export function subscribeAdminSessionEvents(
  adminEventClient: AdminEventClient,
  handlers: AdminSessionEventSyncHandlers,
): AdminEventSubscription {
  const lastLogSignatures = new Map<string, string>();

  return adminEventClient.subscribe({
    onEvent: (event) => {
      if (shouldLogAdminEvent(event, lastLogSignatures)) {
        handlers.onLog(AdminLogEntryFactory.fromAdminEvent(event));
      }
      if (event.type === 'sessions.snapshot') {
        handlers.onLoadingChange(false);
        handlers.onError(null);
        handlers.onSessions(event.sessions);
      } else if (event.type === 'admin.error') {
        handlers.onError(event.error);
      } else if (event.type === 'workflow_runs.snapshot') {
        handlers.onWorkflowRunsSnapshot?.(event.workflowRuns);
      } else if (event.type === 'session_files.snapshot') {
        handlers.onSessionFilesSnapshot?.(event.sessionFiles);
      } else if (event.type === 'recordings.snapshot') {
        handlers.onRecordingsSnapshot?.(event.recordings);
      } else if (event.type === 'mcp_delegation.snapshot') {
        handlers.onMcpDelegationSnapshot?.(event.delegations);
      }
    },
    onStatus: (status) => {
      handlers.onLog(AdminLogEntryFactory.fromConnectionStatus(status));
      handlers.onConnectionStatus?.(status);
    },
    onError: (error) => {
      handlers.onError(error.message);
      handlers.onLog(AdminLogEntryFactory.fromStreamError(error));
    },
  });
}

function shouldLogAdminEvent(event: AdminEvent, lastLogSignatures: Map<string, string>): boolean {
  const signature = AdminLogEntryFactory.eventLogSignature(event);
  if (!signature) {
    return true;
  }
  if (lastLogSignatures.get(event.type) === signature) {
    return false;
  }
  lastLogSignatures.set(event.type, signature);
  return true;
}
