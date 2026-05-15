import type { AdminEventConnectionStatus } from '../api/admin-event-client';
import type {
  AdminMcpDelegationSnapshot,
  AdminRecordingsSnapshot,
  AdminSessionFilesSnapshot,
  AdminWorkflowRunSnapshot,
} from '../api/admin-event-snapshots';
import type { SessionResource } from '../api/control-types';
import type { AdminMessageFeedback, AdminMessageVariant } from '../presentation/admin-message-types';

const GLOBAL_MESSAGE_TEST_ID = 'admin-global-message';

export type SessionFilesNotificationResult = {
  readonly counts: ReadonlyMap<string, number>;
  readonly message: AdminMessageFeedback | null;
};

export type RecordingsNotificationResult = {
  readonly snapshots: ReadonlyMap<string, AdminRecordingsSnapshot>;
  readonly message: AdminMessageFeedback | null;
};

export type McpDelegationNotificationResult = {
  readonly snapshots: ReadonlyMap<string, AdminMcpDelegationSnapshot>;
  readonly message: AdminMessageFeedback | null;
};

export type WorkflowRunsNotificationResult = {
  readonly states: ReadonlyMap<string, string>;
  readonly message: AdminMessageFeedback | null;
};

export function globalAdminMessage(
  variant: AdminMessageVariant,
  title: string,
  message: string,
): AdminMessageFeedback {
  return { variant, title, message, testId: GLOBAL_MESSAGE_TEST_ID };
}

export function eventStreamStatusMessage(
  previous: AdminEventConnectionStatus | null,
  current: AdminEventConnectionStatus,
): AdminMessageFeedback | null {
  if (current === 'reconnecting') {
    return globalAdminMessage('warning', 'Admin event stream', 'Reconnecting to the gateway event stream.');
  }
  if (current === 'open' && previous && previous !== 'open' && previous !== 'connecting') {
    return globalAdminMessage('success', 'Admin event stream', 'Gateway event stream reconnected.');
  }
  return null;
}

export function selectedSessionDiffMessage(
  previous: SessionResource | null,
  current: SessionResource | null,
  sessionSnapshotSeen: boolean,
): AdminMessageFeedback | null {
  if (!sessionSnapshotSeen || !previous) {
    return null;
  }
  if (!current) {
    return globalAdminMessage('warning', 'Selected session unavailable', `Session ${shortAdminId(previous.id)} is no longer visible.`);
  }
  if (previous.id !== current.id) {
    return null;
  }
  if (previous.state !== current.state) {
    return globalAdminMessage(
      current.state === 'active' ? 'success' : 'warning',
      'Session state changed',
      `Selected session ${shortAdminId(current.id)} changed from ${previous.state} to ${current.state}.`,
    );
  }
  const previousClients = previous.status.connection_counts.total_clients;
  const currentClients = current.status.connection_counts.total_clients;
  if (previousClients === 0 && currentClients > 0) {
    return globalAdminMessage('info', 'Session clients connected', `${currentClients} live client${currentClients === 1 ? '' : 's'} attached to the selected session.`);
  }
  if (previousClients > 0 && currentClients === 0) {
    return globalAdminMessage('info', 'Session clients disconnected', 'No live clients remain attached to the selected session.');
  }
  return null;
}

export function sessionFilesSnapshotMessage(
  selectedSessionId: string | null,
  snapshot: readonly AdminSessionFilesSnapshot[],
  previousCounts: ReadonlyMap<string, number>,
): SessionFilesNotificationResult {
  const counts = new Map(snapshot.map((entry) => [entry.sessionId, entry.fileCount]));
  const selectedSnapshot = selectedSessionId ? snapshot.find((entry) => entry.sessionId === selectedSessionId) : null;
  const previousCount = selectedSessionId ? previousCounts.get(selectedSessionId) : undefined;
  if (!selectedSessionId || !selectedSnapshot || previousCount === undefined || selectedSnapshot.fileCount <= previousCount) {
    return { counts, message: null };
  }
  const delta = selectedSnapshot.fileCount - previousCount;
  return {
    counts,
    message: globalAdminMessage(
      'info',
      'Session files updated',
      `${delta} new runtime file${delta === 1 ? '' : 's'} recorded for the selected session.`,
    ),
  };
}

export function recordingsSnapshotMessage(
  selectedSessionId: string | null,
  snapshot: readonly AdminRecordingsSnapshot[],
  previousSnapshots: ReadonlyMap<string, AdminRecordingsSnapshot>,
): RecordingsNotificationResult {
  const snapshots = new Map(snapshot.map((entry) => [entry.sessionId, entry]));
  const selectedSnapshot = selectedSessionId ? snapshot.find((entry) => entry.sessionId === selectedSessionId) : null;
  const previous = selectedSessionId ? previousSnapshots.get(selectedSessionId) : undefined;
  if (!selectedSessionId || !selectedSnapshot || !previous) {
    return { snapshots, message: null };
  }
  if (selectedSnapshot.activeCount > previous.activeCount) {
    return {
      snapshots,
      message: globalAdminMessage('info', 'Recording updated', 'Recording activity started for the selected session.'),
    };
  }
  if (selectedSnapshot.readyCount > previous.readyCount) {
    return {
      snapshots,
      message: globalAdminMessage('success', 'Recording ready', 'A recording segment is ready for the selected session.'),
    };
  }
  if (
    selectedSnapshot.activeCount < previous.activeCount
    || (selectedSnapshot.recordingCount > previous.recordingCount && selectedSnapshot.readyCount <= previous.readyCount)
  ) {
    return {
      snapshots,
      message: globalAdminMessage('warning', 'Recording changed', 'Recording activity ended without a new ready segment for the selected session.'),
    };
  }
  return { snapshots, message: null };
}

export function mcpDelegationSnapshotMessage(
  selectedSessionId: string | null,
  snapshot: readonly AdminMcpDelegationSnapshot[],
  previousSnapshots: ReadonlyMap<string, AdminMcpDelegationSnapshot>,
): McpDelegationNotificationResult {
  const snapshots = new Map(snapshot.map((entry) => [entry.sessionId, entry]));
  const selectedSnapshot = selectedSessionId ? snapshot.find((entry) => entry.sessionId === selectedSessionId) : null;
  const previous = selectedSessionId ? previousSnapshots.get(selectedSessionId) : undefined;
  if (!selectedSessionId || !selectedSnapshot || !previous) {
    return { snapshots, message: null };
  }
  if (selectedSnapshot.delegatedClientId !== previous.delegatedClientId) {
    return {
      snapshots,
      message: globalAdminMessage(
        selectedSnapshot.delegatedClientId ? 'success' : 'warning',
        'MCP delegation changed',
        selectedSnapshot.delegatedClientId
          ? 'MCP was delegated to the selected session.'
          : 'MCP delegation was removed from the selected session.',
      ),
    };
  }
  if (selectedSnapshot.mcpOwner !== previous.mcpOwner) {
    return {
      snapshots,
      message: globalAdminMessage(
        selectedSnapshot.mcpOwner ? 'info' : 'warning',
        'MCP ownership changed',
        selectedSnapshot.mcpOwner ? 'MCP now owns the selected session.' : 'MCP no longer owns the selected session.',
      ),
    };
  }
  return { snapshots, message: null };
}

export function workflowRunsSnapshotMessage(
  selectedSessionId: string | null,
  runs: readonly AdminWorkflowRunSnapshot[],
  previousStates: ReadonlyMap<string, string>,
): WorkflowRunsNotificationResult {
  const states = new Map(runs.map((run) => [run.id, run.state]));
  if (!selectedSessionId) {
    return { states, message: null };
  }
  let message: AdminMessageFeedback | null = null;
  for (const run of runs.filter((entry) => entry.sessionId === selectedSessionId)) {
    const previousState = previousStates.get(run.id);
    if (!previousState || previousState === run.state) {
      continue;
    }
    if (run.state === 'awaiting_input') {
      message = globalAdminMessage('warning', 'Workflow needs input', `Workflow run ${shortAdminId(run.id)} is waiting for operator input.`);
    } else if (isTerminalWorkflowState(run.state)) {
      message = globalAdminMessage(
        run.state === 'succeeded' ? 'success' : 'error',
        'Workflow finished',
        `Workflow run ${shortAdminId(run.id)} ${run.state}.`,
      );
    } else if (run.state === 'running') {
      message = globalAdminMessage('info', 'Workflow running', `Workflow run ${shortAdminId(run.id)} is running.`);
    }
  }
  return { states, message };
}

export function workflowFollowMessage(run: AdminWorkflowRunSnapshot): AdminMessageFeedback {
  return globalAdminMessage(
    'info',
    'Following workflow run',
    `Connecting to workflow run ${shortAdminId(run.id)} on session ${shortAdminId(run.sessionId)}.`,
  );
}

export function shortAdminId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}

function isTerminalWorkflowState(state: string): boolean {
  return state === 'succeeded' || state === 'failed' || state === 'cancelled' || state === 'timed_out';
}
