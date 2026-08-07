import { describe, expect, it } from 'vitest';

import { AdminEventMapper } from './admin-events';
import { AdminEventStreamAccessMapper } from './admin-event-stream-access';

const mapper = new AdminEventMapper((payload) => {
  const object = payload as Record<string, unknown>;
  return { id: String(object.id) };
});

describe('AdminEventMapper', () => {
  it('maps all supported session-scoped snapshot types', () => {
    const session = mapper.toEvent(event('sessions.snapshot', { sessions: [{ id: 'session-1' }] }));
    const runs = mapper.toEvent(event('workflow_runs.snapshot', {
      workflow_runs: [{ id: 'run-1', session_id: 'session-1', state: 'running', updated_at: '2026-08-07T10:00:00Z' }],
    }));
    const files = mapper.toEvent(event('session_files.snapshot', {
      session_files: [{ session_id: 'session-1', file_count: 2, latest_updated_at: null }],
    }));
    const recordings = mapper.toEvent(event('recordings.snapshot', {
      recordings: [{
        session_id: 'session-1',
        recording_count: 2,
        active_count: 1,
        ready_count: 1,
        latest_updated_at: '2026-08-07T10:00:00Z',
      }],
    }));
    const mcp = mapper.toEvent(event('mcp_delegation.snapshot', {
      mcp_delegations: [{
        session_id: 'session-1',
        delegated_client_id: null,
        delegated_issuer: null,
        mcp_owner: false,
        updated_at: '2026-08-07T10:00:00Z',
      }],
    }));
    const error = mapper.toEvent(event('admin.error', { error: 'snapshot failed' }));

    expect(session.type === 'sessions.snapshot' && session.sessions[0]?.id).toBe('session-1');
    expect(runs.type === 'workflow_runs.snapshot' && runs.workflowRuns[0]?.id).toBe('run-1');
    expect(files.type === 'session_files.snapshot' && files.sessionFiles[0]?.fileCount).toBe(2);
    expect(recordings.type === 'recordings.snapshot' && recordings.recordings[0]?.activeCount).toBe(1);
    expect(mcp.type === 'mcp_delegation.snapshot' && mcp.delegations[0]?.mcpOwner).toBe(false);
    expect(error.type === 'admin.error' && error.error).toBe('snapshot failed');
  });

  it('rejects unsupported events and malformed collection data', () => {
    expect(() => mapper.toEvent(event('unsupported', {}))).toThrow('unsupported admin event type');
    expect(() => mapper.toEvent(event('sessions.snapshot', { sessions: {} }))).toThrow('must be an array');
    expect(() => mapper.toEvent(null)).toThrow('must be an object');
  });
});

describe('AdminEventStreamAccessMapper', () => {
  it('accepts only the fixed initial-message authentication contract', () => {
    const access = AdminEventStreamAccessMapper.fromResponse(accessResponse('event-token'));

    expect(access.token).toBe('event-token');
    expect(AdminEventStreamAccessMapper.toWebSocketUrl('https://pane.example/admin-new/'))
      .toBe('wss://pane.example/api/v1/admin/events');
    expect(AdminEventStreamAccessMapper.toWebSocketUrl('http://localhost:8080/'))
      .toBe('ws://localhost:8080/api/v1/admin/events');
  });

  it('rejects missing, malformed, and untrusted descriptors', () => {
    expect(() => AdminEventStreamAccessMapper.fromResponse(null)).toThrow('must be an object');
    expect(() => AdminEventStreamAccessMapper.fromResponse({ token: 'x' })).toThrow('missing websocket metadata');
    expect(() => AdminEventStreamAccessMapper.fromResponse({
      ...accessResponse('event-token'),
      websocket: {
        ...accessResponse('event-token').websocket,
        endpoint_path: '//attacker.example/events',
      },
    })).toThrow('expected contract');
  });
});

function event(event_type: string, value: Record<string, unknown>) {
  return {
    event_type,
    sequence: 1,
    created_at: '2026-08-07T10:00:00Z',
    ...value,
  };
}

function accessResponse(token: string) {
  return {
    token_type: 'admin_event_access_token',
    token,
    websocket: {
      endpoint_path: '/api/v1/admin/events',
      auth_type: 'initial_message',
      authentication_message_type: 'admin.authenticate',
      authenticated_message_type: 'admin.authenticated',
    },
  };
}
