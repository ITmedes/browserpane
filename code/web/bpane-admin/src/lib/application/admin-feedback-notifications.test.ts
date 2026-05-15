import { describe, expect, it } from 'vitest';
import type {
  AdminMcpDelegationSnapshot,
  AdminRecordingsSnapshot,
  AdminWorkflowRunSnapshot,
} from '../api/admin-event-snapshots';
import type { SessionResource } from '../api/control-types';
import {
  eventStreamStatusMessage,
  mcpDelegationSnapshotMessage,
  recordingsSnapshotMessage,
  selectedSessionDiffMessage,
  sessionFilesSnapshotMessage,
  workflowFollowMessage,
  workflowRunsSnapshotMessage,
} from './admin-feedback-notifications';

const SESSION_ID = '019df4d2-f4f7-7b00-9e0c-79683b1c82f6';

describe('admin feedback notifications', () => {
  it('does not notify selected-session diffs before the first event snapshot', () => {
    expect(selectedSessionDiffMessage(session(), session({ state: 'stopped' }), false)).toBeNull();
  });

  it('notifies selected-session state and presence transitions', () => {
    expect(selectedSessionDiffMessage(session(), session({ state: 'stopped' }), true)).toMatchObject({
      variant: 'warning',
      title: 'Session state changed',
      message: 'Selected session 019df4d2...82f6 changed from active to stopped.',
    });

    expect(selectedSessionDiffMessage(
      session({ totalClients: 0 }),
      session({ totalClients: 2 }),
      true,
    )).toMatchObject({
      variant: 'info',
      title: 'Session clients connected',
      message: '2 live clients attached to the selected session.',
    });
  });

  it('notifies selected-session file count increases only after baseline', () => {
    const first = sessionFilesSnapshotMessage(SESSION_ID, [
      { sessionId: SESSION_ID, fileCount: 1, latestUpdatedAt: null },
    ], new Map());
    expect(first.message).toBeNull();

    const second = sessionFilesSnapshotMessage(SESSION_ID, [
      { sessionId: SESSION_ID, fileCount: 3, latestUpdatedAt: null },
    ], first.counts);
    expect(second.message).toMatchObject({
      variant: 'info',
      title: 'Session files updated',
      message: '2 new runtime files recorded for the selected session.',
    });
  });

  it('notifies recording start, ready, and ended-without-ready transitions', () => {
    const baseline = new Map([[SESSION_ID, recording({ activeCount: 0, readyCount: 0, recordingCount: 0 })]]);
    expect(recordingsSnapshotMessage(SESSION_ID, [
      recording({ activeCount: 1, readyCount: 0, recordingCount: 1 }),
    ], baseline).message).toMatchObject({
      variant: 'info',
      title: 'Recording updated',
    });

    expect(recordingsSnapshotMessage(SESSION_ID, [
      recording({ activeCount: 0, readyCount: 1, recordingCount: 1 }),
    ], new Map([[SESSION_ID, recording({ activeCount: 1, readyCount: 0, recordingCount: 1 })]])).message).toMatchObject({
      variant: 'success',
      title: 'Recording ready',
    });

    expect(recordingsSnapshotMessage(SESSION_ID, [
      recording({ activeCount: 0, readyCount: 0, recordingCount: 1 }),
    ], new Map([[SESSION_ID, recording({ activeCount: 1, readyCount: 0, recordingCount: 1 })]])).message).toMatchObject({
      variant: 'warning',
      title: 'Recording changed',
    });
  });

  it('notifies MCP delegation and owner changes', () => {
    const previous = new Map([[SESSION_ID, mcp({ delegatedClientId: null, mcpOwner: false })]]);
    expect(mcpDelegationSnapshotMessage(SESSION_ID, [
      mcp({ delegatedClientId: 'bpane-mcp-bridge', mcpOwner: false }),
    ], previous).message).toMatchObject({
      variant: 'success',
      title: 'MCP delegation changed',
    });

    expect(mcpDelegationSnapshotMessage(SESSION_ID, [
      mcp({ delegatedClientId: null, mcpOwner: true }),
    ], previous).message).toMatchObject({
      variant: 'info',
      title: 'MCP ownership changed',
    });
  });

  it('notifies workflow run transitions and follow events', () => {
    const run: AdminWorkflowRunSnapshot = {
      id: 'run-1234567890',
      sessionId: SESSION_ID,
      state: 'awaiting_input',
      updatedAt: '2026-05-04T19:02:00Z',
    };
    expect(workflowRunsSnapshotMessage(SESSION_ID, [run], new Map([[run.id, 'running']])).message).toMatchObject({
      variant: 'warning',
      title: 'Workflow needs input',
    });

    expect(workflowFollowMessage(run)).toMatchObject({
      variant: 'info',
      title: 'Following workflow run',
    });
  });

  it('notifies event stream reconnect and recovery', () => {
    expect(eventStreamStatusMessage('open', 'reconnecting')).toMatchObject({
      variant: 'warning',
      title: 'Admin event stream',
    });
    expect(eventStreamStatusMessage('reconnecting', 'open')).toMatchObject({
      variant: 'success',
      title: 'Admin event stream',
    });
  });
});

function session(overrides: { readonly state?: string; readonly totalClients?: number } = {}): SessionResource {
  const totalClients = overrides.totalClients ?? 1;
  return {
    id: SESSION_ID,
    state: overrides.state ?? 'active',
    owner_mode: 'shared',
    idle_timeout_sec: 1800,
    labels: {},
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: '/session',
      auth_type: 'session_connect_ticket',
      compatibility_mode: 'session_runtime_pool',
    },
    runtime: {
      binding: 'docker_runtime_pool',
      compatibility_mode: 'session_runtime_pool',
    },
    status: {
      runtime_state: 'running',
      presence_state: totalClients > 0 ? 'connected' : 'idle',
      connection_counts: {
        interactive_clients: totalClients,
        owner_clients: totalClients,
        viewer_clients: 0,
        recorder_clients: 0,
        automation_clients: 0,
        total_clients: totalClients,
      },
      stop_eligibility: { allowed: totalClients === 0, blockers: [] },
    },
    created_at: '2026-05-04T19:00:00Z',
    updated_at: '2026-05-04T19:01:00Z',
  };
}

function recording(overrides: Partial<AdminRecordingsSnapshot>): AdminRecordingsSnapshot {
  return {
    sessionId: SESSION_ID,
    recordingCount: overrides.recordingCount ?? 0,
    activeCount: overrides.activeCount ?? 0,
    readyCount: overrides.readyCount ?? 0,
    latestUpdatedAt: null,
  };
}

function mcp(overrides: Partial<AdminMcpDelegationSnapshot>): AdminMcpDelegationSnapshot {
  return {
    sessionId: SESSION_ID,
    delegatedClientId: overrides.delegatedClientId ?? null,
    delegatedIssuer: overrides.delegatedIssuer ?? null,
    mcpOwner: overrides.mcpOwner ?? false,
    updatedAt: '2026-05-04T19:01:00Z',
  };
}
