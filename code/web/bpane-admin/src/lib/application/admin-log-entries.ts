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
  static fromAdminEvent(event: AdminEvent): AdminLogEntry {
    if (event.type === 'sessions.snapshot') {
      return entry({
        timestamp: event.createdAt,
        level: 'info',
        source: 'gateway',
        message: `Gateway session snapshot #${event.sequence}: ${event.sessions.length} visible sessions.`,
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
