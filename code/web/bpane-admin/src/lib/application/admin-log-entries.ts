import type { AdminEvent } from '../api/admin-event-mapper';
import type { AdminEventConnectionStatus } from '../api/admin-event-client';
import type { SessionResource } from '../api/control-types';
import type { AdminLogEntry } from '../presentation/logs-view-model';

const MAX_LOG_ENTRIES = 120;

type LogMetadata = {
  readonly id?: string;
  readonly now?: Date;
};

type UiStateLogInput = {
  readonly selectedSession: SessionResource | null;
  readonly browserConnected: boolean;
  readonly sessionCount: number;
};

export class AdminLogEntryFactory {
  static eventLogSignature(event: AdminEvent): string | null {
    if (event.type === 'sessions.snapshot') {
      return stableSignature([
        event.type,
        event.sessions
          .map((session) => [
            session.id,
            session.state ?? null,
            session.owner_mode ?? null,
            sortedRecordEntries(session.labels),
            session.automation_delegate?.client_id ?? null,
            session.automation_delegate?.issuer ?? null,
            session.automation_delegate?.display_name ?? null,
            session.runtime?.binding ?? null,
            session.runtime?.compatibility_mode ?? null,
            session.runtime?.cdp_endpoint ?? null,
            session.status?.runtime_state ?? null,
            session.status?.runtime_resume_mode ?? null,
            session.status?.presence_state ?? null,
            session.status?.connection_counts?.interactive_clients ?? null,
            session.status?.connection_counts?.owner_clients ?? null,
            session.status?.connection_counts?.viewer_clients ?? null,
            session.status?.connection_counts?.recorder_clients ?? null,
            session.status?.connection_counts?.automation_clients ?? null,
            session.status?.connection_counts?.total_clients ?? null,
            session.status?.stop_eligibility?.allowed ?? null,
            session.status?.stop_eligibility?.blockers
              ?.map((blocker) => [blocker.kind, blocker.count])
              .sort(compareSignatureParts) ?? [],
            session.runtime_released_at ?? null,
            session.stopped_at ?? null,
          ])
          .sort(compareSignatureParts),
      ]);
    }
    if (event.type === 'workflow_runs.snapshot') {
      return stableSignature([
        event.type,
        event.workflowRuns
          .map((run) => [run.id, run.sessionId, run.state, run.updatedAt])
          .sort(compareSignatureParts),
      ]);
    }
    if (event.type === 'session_files.snapshot') {
      return stableSignature([
        event.type,
        event.sessionFiles
          .map((session) => [session.sessionId, session.fileCount, session.latestUpdatedAt])
          .sort(compareSignatureParts),
      ]);
    }
    if (event.type === 'recordings.snapshot') {
      return stableSignature([
        event.type,
        event.recordings
          .map((session) => [
            session.sessionId,
            session.recordingCount,
            session.activeCount,
            session.readyCount,
            session.latestUpdatedAt,
          ])
          .sort(compareSignatureParts),
      ]);
    }
    if (event.type === 'mcp_delegation.snapshot') {
      return stableSignature([
        event.type,
        event.delegations
          .map((delegation) => [
            delegation.sessionId,
            delegation.delegatedClientId,
            delegation.delegatedIssuer,
            delegation.mcpOwner,
          ])
          .sort(compareSignatureParts),
      ]);
    }
    return null;
  }

  static fromAdminEvent(event: AdminEvent): AdminLogEntry {
    if (event.type === 'sessions.snapshot') {
      return entry({
        timestamp: event.createdAt,
        level: 'info',
        source: 'gateway',
        message: `Gateway session snapshot #${event.sequence}: ${event.sessions.length} visible sessions.`,
      });
    }
    if (event.type === 'workflow_runs.snapshot') {
      const active = event.workflowRuns.filter((run) => !TERMINAL_WORKFLOW_STATES.has(run.state)).length;
      return entry({
        timestamp: event.createdAt,
        level: 'info',
        source: 'gateway',
        message: `Gateway workflow snapshot #${event.sequence}: ${event.workflowRuns.length} runs, ${active} active.`,
      });
    }
    if (event.type === 'session_files.snapshot') {
      const fileCount = event.sessionFiles.reduce((sum, session) => sum + session.fileCount, 0);
      return entry({
        timestamp: event.createdAt,
        level: 'info',
        source: 'gateway',
        message: `Gateway session file snapshot #${event.sequence}: ${fileCount} files across ${event.sessionFiles.length} sessions.`,
      });
    }
    if (event.type === 'recordings.snapshot') {
      const recordingCount = event.recordings.reduce((sum, session) => sum + session.recordingCount, 0);
      const activeCount = event.recordings.reduce((sum, session) => sum + session.activeCount, 0);
      return entry({
        timestamp: event.createdAt,
        level: 'info',
        source: 'gateway',
        message: `Gateway recording snapshot #${event.sequence}: ${recordingCount} segments, ${activeCount} active.`,
      });
    }
    if (event.type === 'mcp_delegation.snapshot') {
      const delegatedCount = event.delegations.filter((entry) => entry.delegatedClientId).length;
      const mcpOwnerCount = event.delegations.filter((entry) => entry.mcpOwner).length;
      return entry({
        timestamp: event.createdAt,
        level: 'info',
        source: 'gateway',
        message: `Gateway MCP delegation snapshot #${event.sequence}: ${delegatedCount} delegated, ${mcpOwnerCount} MCP-owned.`,
      });
    }
    return entry({
      timestamp: event.createdAt,
      level: 'warn',
      source: 'gateway',
      message: `Gateway event stream error #${event.sequence}: ${event.error}`,
    });
  }

  static fromConnectionStatus(
    status: AdminEventConnectionStatus,
    metadata: LogMetadata = {},
  ): AdminLogEntry {
    return entry({
      level: status === 'open' || status === 'closed' ? 'info' : 'warn',
      source: 'ui',
      message: `Admin event stream ${status}.`,
      metadata,
    });
  }

  static fromStreamError(error: Error, metadata: LogMetadata = {}): AdminLogEntry {
    return entry({
      level: 'warn',
      source: 'ui',
      message: `Admin event stream error: ${error.message}`,
      metadata,
    });
  }

  static fromUiState(input: UiStateLogInput, metadata: LogMetadata = {}): AdminLogEntry {
    const connection = input.browserConnected ? 'browser connected' : 'browser disconnected';
    return entry({
      level: 'info',
      source: 'ui',
      message: input.selectedSession
        ? `Selected ${input.selectedSession.id} is ${input.selectedSession.state}, ${connection}.`
        : `No session selected, ${input.sessionCount} visible sessions.`,
      metadata,
    });
  }

  static append(
    entries: readonly AdminLogEntry[],
    ...nextEntries: readonly AdminLogEntry[]
  ): readonly AdminLogEntry[] {
    return [...[...nextEntries].reverse(), ...entries].slice(0, MAX_LOG_ENTRIES);
  }

  static copyText(entries: readonly AdminLogEntry[]): string {
    return entries.map((entry) => `${entry.timestamp} [${entry.source}] ${entry.message}`).join('\n');
  }
}

const TERMINAL_WORKFLOW_STATES = new Set(['succeeded', 'failed', 'cancelled', 'timed_out']);

function sortedRecordEntries(record: Readonly<Record<string, string>> | undefined): readonly [string, string][] {
  return Object.entries(record ?? {}).sort(([left], [right]) => left.localeCompare(right));
}

function stableSignature(value: unknown): string {
  return JSON.stringify(value);
}

function compareSignatureParts(left: readonly unknown[], right: readonly unknown[]): number {
  return JSON.stringify(left).localeCompare(JSON.stringify(right));
}

function entry(input: {
  readonly timestamp?: string;
  readonly level: AdminLogEntry['level'];
  readonly source: AdminLogEntry['source'];
  readonly message: string;
  readonly metadata?: LogMetadata;
}): AdminLogEntry {
  return {
    id: input.metadata?.id ?? crypto.randomUUID(),
    timestamp: input.timestamp ?? input.metadata?.now?.toISOString() ?? new Date().toISOString(),
    level: input.level,
    source: input.source,
    message: input.message,
  };
}
