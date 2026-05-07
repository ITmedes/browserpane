import { describe, expect, it } from 'vitest';
import { AdminLogEntryFactory } from './admin-log-entries';

const NOW = new Date('2026-05-04T19:02:00Z');

describe('AdminLogEntryFactory', () => {
  it('turns session snapshot events into gateway log entries', () => {
    const entry = AdminLogEntryFactory.fromAdminEvent({
      type: 'sessions.snapshot',
      sequence: 7,
      createdAt: NOW.toISOString(),
      sessions: [{ id: 'session-a' }, { id: 'session-b' }],
    } as never);

    expect(entry.source).toBe('gateway');
    expect(entry.level).toBe('info');
    expect(entry.timestamp).toBe(NOW.toISOString());
    expect(entry.message).toBe('Gateway session snapshot #7: 2 visible sessions.');
  });

  it('preserves local stream diagnostics separately from gateway events', () => {
    const entry = AdminLogEntryFactory.fromConnectionStatus('reconnecting', {
      id: 'log-1',
      now: NOW,
    });

    expect(entry).toEqual({
      id: 'log-1',
      timestamp: NOW.toISOString(),
      level: 'warn',
      source: 'ui',
      message: 'Admin event stream reconnecting.',
    });
  });

  it('turns workflow run snapshot events into gateway log entries', () => {
    const entry = AdminLogEntryFactory.fromAdminEvent({
      type: 'workflow_runs.snapshot',
      sequence: 8,
      createdAt: NOW.toISOString(),
      workflowRuns: [
        { id: 'run-a', sessionId: 'session-a', state: 'running', updatedAt: NOW.toISOString() },
        { id: 'run-b', sessionId: 'session-a', state: 'succeeded', updatedAt: NOW.toISOString() },
      ],
    });

    expect(entry.source).toBe('gateway');
    expect(entry.message).toBe('Gateway workflow snapshot #8: 2 runs, 1 active.');
  });

  it('bounds appended log history to newest entries', () => {
    const entries = Array.from({ length: 121 }, (_, index) =>
      AdminLogEntryFactory.fromConnectionStatus('open', {
        id: `entry-${index}`,
        now: NOW,
      }),
    );

    const bounded = AdminLogEntryFactory.append([], ...entries);

    expect(bounded).toHaveLength(120);
    expect(bounded[0]?.id).toBe('entry-120');
    expect(bounded.at(-1)?.id).toBe('entry-1');
  });
});
