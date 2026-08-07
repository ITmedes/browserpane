export type AdminEventType =
  | 'sessions.snapshot'
  | 'workflow_runs.snapshot'
  | 'session_files.snapshot'
  | 'recordings.snapshot'
  | 'mcp_delegation.snapshot'
  | 'admin.error';

export type AdminSessionsSnapshotEvent<SessionResource> = {
  readonly type: 'sessions.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly sessions: readonly SessionResource[];
};

export type AdminWorkflowRunSnapshot = {
  readonly id: string;
  readonly sessionId: string;
  readonly state: string;
  readonly updatedAt: string;
};

export type AdminWorkflowRunsSnapshotEvent = {
  readonly type: 'workflow_runs.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly workflowRuns: readonly AdminWorkflowRunSnapshot[];
};

export type AdminSessionFilesSnapshot = {
  readonly sessionId: string;
  readonly fileCount: number;
  readonly latestUpdatedAt: string | null;
};

export type AdminSessionFilesSnapshotEvent = {
  readonly type: 'session_files.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly sessionFiles: readonly AdminSessionFilesSnapshot[];
};

export type AdminRecordingsSnapshot = {
  readonly sessionId: string;
  readonly recordingCount: number;
  readonly activeCount: number;
  readonly readyCount: number;
  readonly latestUpdatedAt: string | null;
};

export type AdminRecordingsSnapshotEvent = {
  readonly type: 'recordings.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly recordings: readonly AdminRecordingsSnapshot[];
};

export type AdminMcpDelegationSnapshot = {
  readonly sessionId: string;
  readonly delegatedClientId: string | null;
  readonly delegatedIssuer: string | null;
  readonly mcpOwner: boolean;
  readonly updatedAt: string;
};

export type AdminMcpDelegationSnapshotEvent = {
  readonly type: 'mcp_delegation.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly delegations: readonly AdminMcpDelegationSnapshot[];
};

export type AdminErrorEvent = {
  readonly type: 'admin.error';
  readonly sequence: number;
  readonly createdAt: string;
  readonly error: string;
};

export type AdminEvent<SessionResource> =
  | AdminSessionsSnapshotEvent<SessionResource>
  | AdminWorkflowRunsSnapshotEvent
  | AdminSessionFilesSnapshotEvent
  | AdminRecordingsSnapshotEvent
  | AdminMcpDelegationSnapshotEvent
  | AdminErrorEvent;

export type AdminSessionMapper<SessionResource> = (payload: unknown) => SessionResource;

export class AdminEventMapper<SessionResource> {
  readonly #sessionMapper: AdminSessionMapper<SessionResource>;

  constructor(sessionMapper: AdminSessionMapper<SessionResource>) {
    this.#sessionMapper = sessionMapper;
  }

  toEvent(payload: unknown): AdminEvent<SessionResource> {
    const object = expectRecord(payload, 'admin event');
    const eventType = expectString(object.event_type, 'admin event event_type');
    const envelope = {
      sequence: expectNumber(object.sequence, `${eventType} event sequence`),
      createdAt: expectString(object.created_at, `${eventType} event created_at`),
    };
    if (eventType === 'sessions.snapshot') {
      return {
        type: eventType,
        ...envelope,
        sessions: expectArray(object.sessions, 'sessions.snapshot event sessions').map(this.#sessionMapper),
      };
    }
    if (eventType === 'workflow_runs.snapshot') {
      return {
        type: eventType,
        ...envelope,
        workflowRuns: expectArray(object.workflow_runs, 'workflow_runs.snapshot event workflow_runs')
          .map(toWorkflowRunSnapshot),
      };
    }
    if (eventType === 'session_files.snapshot') {
      return {
        type: eventType,
        ...envelope,
        sessionFiles: expectArray(object.session_files, 'session_files.snapshot event session_files')
          .map(toSessionFilesSnapshot),
      };
    }
    if (eventType === 'recordings.snapshot') {
      return {
        type: eventType,
        ...envelope,
        recordings: expectArray(object.recordings, 'recordings.snapshot event recordings')
          .map(toRecordingSnapshot),
      };
    }
    if (eventType === 'mcp_delegation.snapshot') {
      return {
        type: eventType,
        ...envelope,
        delegations: expectArray(object.mcp_delegations, 'mcp_delegation.snapshot event mcp_delegations')
          .map(toMcpDelegationSnapshot),
      };
    }
    if (eventType === 'admin.error') {
      return {
        type: eventType,
        ...envelope,
        error: expectString(object.error, 'admin.error event error'),
      };
    }
    throw new Error(`unsupported admin event type: ${eventType}`);
  }
}

function toWorkflowRunSnapshot(payload: unknown): AdminWorkflowRunSnapshot {
  const object = expectRecord(payload, 'workflow run snapshot');
  return {
    id: expectString(object.id, 'workflow run snapshot id'),
    sessionId: expectString(object.session_id, 'workflow run snapshot session_id'),
    state: expectString(object.state, 'workflow run snapshot state'),
    updatedAt: expectString(object.updated_at, 'workflow run snapshot updated_at'),
  };
}

function toSessionFilesSnapshot(payload: unknown): AdminSessionFilesSnapshot {
  const object = expectRecord(payload, 'session files snapshot');
  return {
    sessionId: expectString(object.session_id, 'session files snapshot session_id'),
    fileCount: expectNumber(object.file_count, 'session files snapshot file_count'),
    latestUpdatedAt: optionalString(object.latest_updated_at, 'session files snapshot latest_updated_at'),
  };
}

function toRecordingSnapshot(payload: unknown): AdminRecordingsSnapshot {
  const object = expectRecord(payload, 'recordings snapshot');
  return {
    sessionId: expectString(object.session_id, 'recordings snapshot session_id'),
    recordingCount: expectNumber(object.recording_count, 'recordings snapshot recording_count'),
    activeCount: expectNumber(object.active_count, 'recordings snapshot active_count'),
    readyCount: expectNumber(object.ready_count, 'recordings snapshot ready_count'),
    latestUpdatedAt: optionalString(object.latest_updated_at, 'recordings snapshot latest_updated_at'),
  };
}

function toMcpDelegationSnapshot(payload: unknown): AdminMcpDelegationSnapshot {
  const object = expectRecord(payload, 'mcp delegation snapshot');
  return {
    sessionId: expectString(object.session_id, 'mcp delegation snapshot session_id'),
    delegatedClientId: optionalString(object.delegated_client_id, 'mcp delegation snapshot delegated_client_id'),
    delegatedIssuer: optionalString(object.delegated_issuer, 'mcp delegation snapshot delegated_issuer'),
    mcpOwner: expectBoolean(object.mcp_owner, 'mcp delegation snapshot mcp_owner'),
    updatedAt: expectString(object.updated_at, 'mcp delegation snapshot updated_at'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null {
  if (value === undefined || value === null) {
    return null;
  }
  return expectString(value, label);
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`${label} must be a finite number`);
  }
  return value;
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new Error(`${label} must be a boolean`);
  }
  return value;
}
