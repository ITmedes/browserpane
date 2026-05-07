import { ControlSessionMapper } from './control-session-mapper';
import type { SessionResource } from './control-types';
import { expectNumber, expectRecord, expectString } from './control-wire';

export type AdminEventType = 'sessions.snapshot' | 'admin.error';

export type AdminSessionsSnapshotEvent = {
  readonly type: 'sessions.snapshot';
  readonly sequence: number;
  readonly createdAt: string;
  readonly sessions: readonly SessionResource[];
};

export type AdminErrorEvent = {
  readonly type: 'admin.error';
  readonly sequence: number;
  readonly createdAt: string;
  readonly error: string;
};

export type AdminEvent = AdminSessionsSnapshotEvent | AdminErrorEvent;

export class AdminEventMapper {
  static toEvent(payload: unknown): AdminEvent {
    const object = expectRecord(payload, 'admin event');
    const eventType = expectString(object.event_type, 'admin event event_type');
    if (eventType === 'sessions.snapshot') {
      return toSessionsSnapshotEvent(object);
    }
    if (eventType === 'admin.error') {
      return toAdminErrorEvent(object);
    }
    throw new Error(`unsupported admin event type: ${eventType}`);
  }
}

function toSessionsSnapshotEvent(object: Record<string, unknown>): AdminSessionsSnapshotEvent {
  const sessions = object.sessions;
  if (!Array.isArray(sessions)) {
    throw new Error('sessions.snapshot event sessions must be an array');
  }
  return {
    type: 'sessions.snapshot',
    sequence: expectNumber(object.sequence, 'sessions.snapshot event sequence'),
    createdAt: expectString(object.created_at, 'sessions.snapshot event created_at'),
    sessions: sessions.map((session) => ControlSessionMapper.toSessionResource(session)),
  };
}

function toAdminErrorEvent(object: Record<string, unknown>): AdminErrorEvent {
  return {
    type: 'admin.error',
    sequence: expectNumber(object.sequence, 'admin.error event sequence'),
    createdAt: expectString(object.created_at, 'admin.error event created_at'),
    error: expectString(object.error, 'admin.error event error'),
  };
}
