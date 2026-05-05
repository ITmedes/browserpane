import type {
  SessionConnectionCounts,
  SessionConnectInfo,
  SessionAccessTokenResponse,
  SessionListResponse,
  SessionResource,
  SessionRuntimeInfo,
  SessionStatusSummary,
  SessionStopBlocker,
  SessionStopEligibility,
} from './control-types';
import {
  expectBoolean,
  expectNumber,
  expectRecord,
  expectString,
  optionalString,
} from './control-wire';

export class ControlSessionMapper {
  static toSessionList(payload: unknown): SessionListResponse {
    const object = expectRecord(payload, 'session list response');
    const sessions = object.sessions;
    if (!Array.isArray(sessions)) {
      throw new Error('session list response must contain a sessions array');
    }
    return {
      sessions: sessions.map((session) => this.toSessionResource(session)),
    };
  }

  static toSessionResource(payload: unknown): SessionResource {
    const object = expectRecord(payload, 'session resource');
    const stoppedAt = optionalString(object.stopped_at, 'session resource stopped_at');
    return {
      id: expectString(object.id, 'session resource id'),
      state: expectString(object.state, 'session resource state'),
      owner_mode: expectString(object.owner_mode, 'session resource owner_mode'),
      connect: toConnectInfo(object.connect),
      runtime: toRuntimeInfo(object.runtime),
      status: toStatusSummary(object.status),
      created_at: expectString(object.created_at, 'session resource created_at'),
      updated_at: expectString(object.updated_at, 'session resource updated_at'),
      ...(stoppedAt !== undefined ? { stopped_at: stoppedAt } : {}),
    };
  }

  static toSessionAccessTokenResponse(payload: unknown): SessionAccessTokenResponse {
    const object = expectRecord(payload, 'session access token response');
    return {
      session_id: expectString(object.session_id, 'session access token session_id'),
      token_type: expectString(object.token_type, 'session access token token_type'),
      token: expectString(object.token, 'session access token token'),
      expires_at: expectString(object.expires_at, 'session access token expires_at'),
      connect: toConnectInfo(object.connect),
    };
  }
}

function toConnectInfo(value: unknown): SessionConnectInfo {
  const object = expectRecord(value, 'session resource connect');
  const ticketPath = optionalString(object.ticket_path, 'session connect ticket_path');
  return {
    gateway_url: expectString(object.gateway_url, 'session connect gateway_url'),
    transport_path: expectString(object.transport_path, 'session connect transport_path'),
    auth_type: expectString(object.auth_type, 'session connect auth_type'),
    ...(ticketPath !== undefined ? { ticket_path: ticketPath } : {}),
    compatibility_mode: expectString(object.compatibility_mode, 'session connect compatibility_mode'),
  };
}

function toRuntimeInfo(value: unknown): SessionRuntimeInfo {
  const object = expectRecord(value, 'session resource runtime');
  const cdpEndpoint = optionalString(object.cdp_endpoint, 'session runtime cdp_endpoint');
  return {
    binding: expectString(object.binding, 'session runtime binding'),
    compatibility_mode: expectString(object.compatibility_mode, 'session runtime compatibility_mode'),
    ...(cdpEndpoint !== undefined ? { cdp_endpoint: cdpEndpoint } : {}),
  };
}

function toStatusSummary(value: unknown): SessionStatusSummary {
  const object = expectRecord(value, 'session resource status');
  return {
    runtime_state: expectString(object.runtime_state, 'session status runtime_state'),
    presence_state: expectString(object.presence_state, 'session status presence_state'),
    connection_counts: toConnectionCounts(object.connection_counts),
    stop_eligibility: toStopEligibility(object.stop_eligibility),
  };
}

function toConnectionCounts(value: unknown): SessionConnectionCounts {
  const object = expectRecord(value, 'session status connection_counts');
  return {
    interactive_clients: expectNumber(object.interactive_clients, 'interactive_clients'),
    owner_clients: expectNumber(object.owner_clients, 'owner_clients'),
    viewer_clients: expectNumber(object.viewer_clients, 'viewer_clients'),
    recorder_clients: expectNumber(object.recorder_clients, 'recorder_clients'),
    automation_clients: expectNumber(object.automation_clients, 'automation_clients'),
    total_clients: expectNumber(object.total_clients, 'total_clients'),
  };
}

function toStopEligibility(value: unknown): SessionStopEligibility {
  const object = expectRecord(value, 'session status stop_eligibility');
  const blockers = object.blockers;
  if (!Array.isArray(blockers)) {
    throw new Error('session stop eligibility blockers must be an array');
  }
  return {
    allowed: expectBoolean(object.allowed, 'session stop eligibility allowed'),
    blockers: blockers.map((blocker) => toStopBlocker(blocker)),
  };
}

function toStopBlocker(value: unknown): SessionStopBlocker {
  const object = expectRecord(value, 'session stop blocker');
  return {
    kind: expectString(object.kind, 'session stop blocker kind'),
    count: expectNumber(object.count, 'session stop blocker count'),
  };
}
