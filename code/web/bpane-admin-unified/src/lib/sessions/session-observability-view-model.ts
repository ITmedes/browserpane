import type { AdminEvent } from '$lib/api/admin-events';
import type { AdminEventConnectionStatus } from '$lib/api/admin-event-client';
import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type {
  AdminMcpDelegationSnapshot,
  AdminRecordingsSnapshot,
  AdminSessionFilesSnapshot,
  AdminWorkflowRunSnapshot,
} from '$lib/api/admin-events';
import type { SessionResource, SessionStatus } from './session-types';

export type SessionObservabilityTimelineEntry = {
  readonly id: string;
  readonly source: string;
  readonly title: string;
  readonly detail: string;
  readonly createdAt: string;
  readonly tone: ProjectTone;
};

export type SessionObservabilityEvidence = {
  readonly session: SessionResource;
  readonly liveStatus: SessionStatus | null;
  readonly workflowRuns: readonly AdminWorkflowRunSnapshot[];
  readonly sessionFiles: AdminSessionFilesSnapshot | null;
  readonly recordings: AdminRecordingsSnapshot | null;
  readonly mcpDelegation: AdminMcpDelegationSnapshot | null;
  readonly lastEventAt: string | null;
  readonly timeline: readonly SessionObservabilityTimelineEntry[];
};

export type SessionObservabilityFact = {
  readonly label: string;
  readonly value: string;
  readonly tone: ProjectTone;
  readonly testId: string;
};

export type SessionObservabilityModel = {
  readonly streamLabel: string;
  readonly streamTone: ProjectTone;
  readonly streamDescription: string;
  readonly facts: readonly SessionObservabilityFact[];
  readonly timeline: readonly SessionObservabilityTimelineEntry[];
};

const TIMELINE_LIMIT = 40;
const ACTIVE_WORKFLOW_STATES = new Set(['pending', 'queued', 'starting', 'running', 'awaiting_input', 'cancelling']);

export function createSessionObservabilityEvidence(
  session: SessionResource,
  liveStatus: SessionStatus | null,
): SessionObservabilityEvidence {
  return {
    session,
    liveStatus,
    workflowRuns: [],
    sessionFiles: null,
    recordings: null,
    mcpDelegation: null,
    lastEventAt: null,
    timeline: [],
  };
}

export function applySessionAdminEvent(
  evidence: SessionObservabilityEvidence,
  event: AdminEvent,
  sessionId: string,
): SessionObservabilityEvidence {
  const entry = timelineEntry(event, sessionId);
  const nextTimeline = entry
    ? [entry, ...evidence.timeline].slice(0, TIMELINE_LIMIT)
    : evidence.timeline;
  const base = {
    ...evidence,
    lastEventAt: entry ? event.createdAt : evidence.lastEventAt,
    timeline: nextTimeline,
  };

  if (event.type === 'sessions.snapshot') {
    return {
      ...base,
      session: event.sessions.find((session) => session.id === sessionId) ?? evidence.session,
    };
  }
  if (event.type === 'workflow_runs.snapshot') {
    return {
      ...base,
      workflowRuns: event.workflowRuns.filter((run) => run.sessionId === sessionId),
    };
  }
  if (event.type === 'session_files.snapshot') {
    return {
      ...base,
      sessionFiles: event.sessionFiles.find((summary) => summary.sessionId === sessionId) ?? null,
    };
  }
  if (event.type === 'recordings.snapshot') {
    return {
      ...base,
      recordings: event.recordings.find((summary) => summary.sessionId === sessionId) ?? null,
    };
  }
  if (event.type === 'mcp_delegation.snapshot') {
    return {
      ...base,
      mcpDelegation: event.delegations.find((summary) => summary.sessionId === sessionId) ?? null,
    };
  }
  return base;
}

export function buildSessionObservabilityModel(
  evidence: SessionObservabilityEvidence,
  streamStatus: AdminEventConnectionStatus,
  streamError: string | null,
): SessionObservabilityModel {
  const connectionCounts = evidence.session.status.connection_counts;
  const activeRuns = evidence.workflowRuns.filter((run) => ACTIVE_WORKFLOW_STATES.has(run.state)).length;
  const stopAllowed = evidence.session.status.stop_eligibility.allowed;
  return {
    streamLabel: streamStatusLabel(streamStatus),
    streamTone: streamStatusTone(streamStatus, streamError),
    streamDescription: streamError ?? streamStatusDescription(streamStatus),
    facts: [
      fact('Runtime', evidence.session.status.runtime_state, runtimeTone(evidence.session.status.runtime_state), 'session-observability-runtime'),
      fact('Presence', evidence.session.status.presence_state, presenceTone(evidence.session.status.presence_state), 'session-observability-presence'),
      fact('Connections', String(connectionCounts.total_clients), connectionCounts.total_clients > 0 ? 'success' : 'neutral', 'session-observability-connections'),
      fact('Resolution', resolutionLabel(evidence.liveStatus), evidence.liveStatus ? 'neutral' : 'warning', 'session-observability-resolution'),
      fact('Stop eligibility', stopAllowed ? 'Allowed' : 'Blocked', stopAllowed ? 'success' : 'warning', 'session-observability-stop'),
      fact('MCP owner', evidence.mcpDelegation?.mcpOwner || evidence.liveStatus?.mcp_owner ? 'Active' : 'Inactive', evidence.mcpDelegation?.mcpOwner || evidence.liveStatus?.mcp_owner ? 'success' : 'neutral', 'session-observability-mcp-owner'),
      fact('Workflow runs', workflowRunLabel(evidence.workflowRuns.length, activeRuns), activeRuns > 0 ? 'warning' : 'neutral', 'session-observability-workflows'),
      fact('Session files', countLabel(evidence.sessionFiles?.fileCount), 'neutral', 'session-observability-files'),
      fact('Recordings', recordingLabel(evidence), recordingTone(evidence), 'session-observability-recordings'),
      fact('Last stream event', evidence.lastEventAt ? formatDateTime(evidence.lastEventAt) : 'Waiting for snapshot', evidence.lastEventAt ? 'success' : 'neutral', 'session-observability-last-event'),
    ],
    timeline: evidence.timeline,
  };
}

function timelineEntry(event: AdminEvent, sessionId: string): SessionObservabilityTimelineEntry | null {
  const base = {
    id: `${event.type}-${event.sequence}-${event.createdAt}`,
    createdAt: event.createdAt,
  };
  if (event.type === 'sessions.snapshot') {
    const session = event.sessions.find((candidate) => candidate.id === sessionId);
    return session
      ? {
          ...base,
          source: 'Session snapshot',
          title: `${session.state} / ${session.status.presence_state}`,
          detail: `${session.status.connection_counts.total_clients} clients, runtime ${session.status.runtime_state}`,
          tone: runtimeTone(session.status.runtime_state),
        }
      : {
          ...base,
          source: 'Session snapshot',
          title: 'Session not visible',
          detail: 'The owner-scoped snapshot no longer contains this session.',
          tone: 'danger',
        };
  }
  if (event.type === 'workflow_runs.snapshot') {
    const runs = event.workflowRuns.filter((run) => run.sessionId === sessionId);
    const active = runs.filter((run) => ACTIVE_WORKFLOW_STATES.has(run.state)).length;
    return {
      ...base,
      source: 'Workflow snapshot',
      title: workflowRunLabel(runs.length, active),
      detail: runs.length === 0 ? 'No workflow runs are associated with this session.' : runStateSummary(runs),
      tone: active > 0 ? 'warning' : 'neutral',
    };
  }
  if (event.type === 'session_files.snapshot') {
    const files = event.sessionFiles.find((summary) => summary.sessionId === sessionId);
    return files ? {
      ...base,
      source: 'Session files snapshot',
      title: `${files.fileCount} bound ${files.fileCount === 1 ? 'file' : 'files'}`,
      detail: files.latestUpdatedAt ? `Latest update ${formatDateTime(files.latestUpdatedAt)}` : 'No file update timestamp reported.',
      tone: 'neutral',
    } : null;
  }
  if (event.type === 'recordings.snapshot') {
    const recordings = event.recordings.find((summary) => summary.sessionId === sessionId);
    return recordings ? {
      ...base,
      source: 'Recording snapshot',
      title: `${recordings.recordingCount} segments, ${recordings.activeCount} active`,
      detail: `${recordings.readyCount} ready for playback or download.`,
      tone: recordings.activeCount > 0 ? 'warning' : 'neutral',
    } : null;
  }
  if (event.type === 'mcp_delegation.snapshot') {
    const delegation = event.delegations.find((summary) => summary.sessionId === sessionId);
    return delegation ? {
      ...base,
      source: 'MCP delegation snapshot',
      title: delegation.delegatedClientId ? `Delegated to ${delegation.delegatedClientId}` : 'No automation delegate',
      detail: delegation.mcpOwner ? 'MCP currently owns browser control.' : 'MCP browser ownership is inactive.',
      tone: delegation.mcpOwner ? 'success' : 'neutral',
    } : null;
  }
  return {
    ...base,
    source: 'Admin event stream',
    title: 'Snapshot error',
    detail: event.error,
    tone: 'danger',
  };
}

function fact(
  label: string,
  value: string,
  tone: ProjectTone,
  testId: string,
): SessionObservabilityFact {
  return { label, value, tone, testId };
}

function resolutionLabel(status: SessionStatus | null): string {
  return status ? `${status.resolution[0]} x ${status.resolution[1]}` : 'Status unavailable';
}

function workflowRunLabel(total: number, active: number): string {
  return `${total} total / ${active} active`;
}

function countLabel(count: number | undefined): string {
  return count === undefined ? 'Waiting for snapshot' : String(count);
}

function recordingLabel(evidence: SessionObservabilityEvidence): string {
  if (evidence.recordings) {
    return `${evidence.recordings.recordingCount} segments / ${evidence.recordings.activeCount} active`;
  }
  return evidence.liveStatus?.recording?.state ?? evidence.session.recording.mode;
}

function recordingTone(evidence: SessionObservabilityEvidence): ProjectTone {
  if ((evidence.recordings?.activeCount ?? 0) > 0) {
    return 'warning';
  }
  if (evidence.liveStatus?.recording?.state === 'failed') {
    return 'danger';
  }
  if ((evidence.recordings?.readyCount ?? 0) > 0) {
    return 'success';
  }
  return 'neutral';
}

function runStateSummary(runs: readonly AdminWorkflowRunSnapshot[]): string {
  const counts = new Map<string, number>();
  for (const run of runs) {
    counts.set(run.state, (counts.get(run.state) ?? 0) + 1);
  }
  return [...counts.entries()].map(([state, count]) => `${count} ${state}`).join(', ');
}

function runtimeTone(runtimeState: string): ProjectTone {
  if (runtimeState === 'running') {
    return 'success';
  }
  if (['failed', 'killed'].includes(runtimeState)) {
    return 'danger';
  }
  return ['starting', 'stopping'].includes(runtimeState) ? 'warning' : 'neutral';
}

function presenceTone(presenceState: string): ProjectTone {
  return presenceState === 'connected' ? 'success' : presenceState === 'disconnecting' ? 'warning' : 'neutral';
}

function streamStatusLabel(status: AdminEventConnectionStatus): string {
  if (status === 'open') {
    return 'Live';
  }
  if (status === 'reconnecting') {
    return 'Reconnecting';
  }
  if (status === 'connecting') {
    return 'Connecting';
  }
  return 'Closed';
}

function streamStatusTone(status: AdminEventConnectionStatus, error: string | null): ProjectTone {
  if (error) {
    return 'danger';
  }
  if (status === 'open') {
    return 'success';
  }
  return status === 'closed' ? 'neutral' : 'warning';
}

function streamStatusDescription(status: AdminEventConnectionStatus): string {
  if (status === 'open') {
    return 'Authenticated owner-scoped snapshots update this view without a page refresh.';
  }
  if (status === 'reconnecting') {
    return 'The client is waiting to mint fresh stream access and reconnect.';
  }
  if (status === 'connecting') {
    return 'Minting short-lived admin-event access and authenticating the WebSocket.';
  }
  return 'The local event subscription is closed.';
}
