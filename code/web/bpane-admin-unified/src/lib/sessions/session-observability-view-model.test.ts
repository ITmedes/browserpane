import { describe, expect, it } from 'vitest';

import { toAdminEvent } from '$lib/api/admin-events';
import { sessionPayload, sessionResource, sessionStatus } from '$lib/test-utils/session-fixtures';
import {
  applySessionAdminEvent,
  buildSessionObservabilityModel,
  createSessionObservabilityEvidence,
} from './session-observability-view-model';

describe('session observability view model', () => {
  it('projects only evidence associated with the selected session', () => {
    let evidence = createSessionObservabilityEvidence(sessionResource(), sessionStatus());
    evidence = applySessionAdminEvent(evidence, toAdminEvent(event('workflow_runs.snapshot', {
      workflow_runs: [
        { id: 'run-1', session_id: 'session-1', state: 'running', updated_at: timestamp(1) },
        { id: 'run-2', session_id: 'session-2', state: 'failed', updated_at: timestamp(1) },
      ],
    })), 'session-1');
    evidence = applySessionAdminEvent(evidence, toAdminEvent(event('session_files.snapshot', {
      session_files: [
        { session_id: 'session-1', file_count: 2, latest_updated_at: timestamp(2) },
        { session_id: 'session-2', file_count: 9, latest_updated_at: timestamp(2) },
      ],
    })), 'session-1');
    evidence = applySessionAdminEvent(evidence, toAdminEvent(event('recordings.snapshot', {
      recordings: [{ session_id: 'session-1', recording_count: 3, active_count: 1, ready_count: 2, latest_updated_at: timestamp(3) }],
    })), 'session-1');
    evidence = applySessionAdminEvent(evidence, toAdminEvent(event('mcp_delegation.snapshot', {
      mcp_delegations: [{
        session_id: 'session-1',
        delegated_client_id: 'bridge',
        delegated_issuer: 'issuer',
        mcp_owner: true,
        updated_at: timestamp(4),
      }],
    })), 'session-1');

    const model = buildSessionObservabilityModel(evidence, 'open', null);
    expect(evidence.workflowRuns.map((run) => run.id)).toEqual(['run-1']);
    expect(fact(model, 'session-observability-workflows').value).toBe('1 total / 1 active');
    expect(fact(model, 'session-observability-files').value).toBe('2');
    expect(fact(model, 'session-observability-recordings').value).toBe('3 segments / 1 active');
    expect(fact(model, 'session-observability-mcp-owner').value).toBe('Active');
    expect(model.timeline).toHaveLength(4);
  });

  it('updates current session state from snapshots and keeps history bounded', () => {
    let evidence = createSessionObservabilityEvidence(sessionResource(), null);
    for (let index = 0; index < 45; index += 1) {
      evidence = applySessionAdminEvent(evidence, toAdminEvent({
        event_type: 'sessions.snapshot',
        sequence: index + 1,
        created_at: timestamp(index),
        sessions: [{
          ...sessionPayload({ state: index === 44 ? 'stopped' : 'ready' }),
          status: {
            ...(sessionPayload().status as Record<string, unknown>),
            runtime_state: index === 44 ? 'stopped' : 'running',
          },
        }],
      }), 'session-1');
    }

    expect(evidence.session.state).toBe('stopped');
    expect(evidence.session.status.runtime_state).toBe('stopped');
    expect(evidence.timeline).toHaveLength(40);
    expect(evidence.timeline[0]?.title).toContain('stopped');
  });

  it('distinguishes stream health from durable historical evidence', () => {
    const evidence = createSessionObservabilityEvidence(sessionResource(), null);

    expect(buildSessionObservabilityModel(evidence, 'connecting', null)).toMatchObject({
      streamLabel: 'Connecting',
      streamTone: 'warning',
    });
    expect(buildSessionObservabilityModel(evidence, 'open', null)).toMatchObject({
      streamLabel: 'Live',
      streamTone: 'success',
    });
    expect(buildSessionObservabilityModel(evidence, 'reconnecting', 'socket failed')).toMatchObject({
      streamLabel: 'Reconnecting',
      streamTone: 'danger',
      streamDescription: 'socket failed',
    });
  });
});

function fact(model: ReturnType<typeof buildSessionObservabilityModel>, testId: string) {
  const value = model.facts.find((candidate) => candidate.testId === testId);
  expect(value).toBeDefined();
  return value!;
}

function event(event_type: string, body: Record<string, unknown>) {
  return { event_type, sequence: 1, created_at: timestamp(0), ...body };
}

function timestamp(offset: number): string {
  return `2026-08-07T10:${String(offset).padStart(2, '0')}:00Z`;
}
