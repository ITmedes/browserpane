import type { AdminEventClient, AdminEventSubscription } from '../api/admin-event-client';
import type { SessionResource } from '../api/control-types';
import type { AdminLogEntry } from '../presentation/logs-view-model';
import { AdminLogEntryFactory } from './admin-log-entries';

type AdminSessionEventSyncHandlers = {
  readonly onSessions: (sessions: readonly SessionResource[]) => void;
  readonly onLoadingChange: (loading: boolean) => void;
  readonly onError: (error: string | null) => void;
  readonly onLog: (entry: AdminLogEntry) => void;
  readonly onSessionFilesSnapshot?: () => void;
};

export function subscribeAdminSessionEvents(
  adminEventClient: AdminEventClient,
  handlers: AdminSessionEventSyncHandlers,
): AdminEventSubscription {
  return adminEventClient.subscribe({
    onEvent: (event) => {
      handlers.onLog(AdminLogEntryFactory.fromAdminEvent(event));
      if (event.type === 'sessions.snapshot') {
        handlers.onLoadingChange(false);
        handlers.onError(null);
        handlers.onSessions(event.sessions);
      } else if (event.type === 'admin.error') {
        handlers.onError(event.error);
      } else if (event.type === 'session_files.snapshot') {
        handlers.onSessionFilesSnapshot?.();
      }
    },
    onStatus: (status) => handlers.onLog(AdminLogEntryFactory.fromConnectionStatus(status)),
    onError: (error) => {
      handlers.onError(error.message);
      handlers.onLog(AdminLogEntryFactory.fromStreamError(error));
    },
  });
}
