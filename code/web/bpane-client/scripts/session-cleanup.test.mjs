import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { SessionCleanup } from './session-cleanup.mjs';

describe('SessionCleanup', () => {
  it('waits for a quiet window and removes sessions that appear during reconciliation', async () => {
    const responses = [
      catalog(session('session-a', 'active')),
      catalog(),
      catalog(session('session-b', 'ready')),
      catalog(),
      catalog(),
    ];
    const killed = [];

    const result = await new SessionCleanup({
      list: async () => responses.shift() ?? catalog(),
      kill: async (sessionId) => killed.push(sessionId),
      quietPasses: 2,
      wait: async () => {},
    }).run();

    assert.deepEqual(killed, ['session-a', 'session-b']);
    assert.deepEqual(result.removedSessionIds, ['session-a', 'session-b']);
  });

  it('deduplicates active session ids and ignores stopped or malformed rows', async () => {
    const killed = [];
    const responses = [
      catalog(
        session('session-a', 'idle'),
        session('session-a', 'active'),
        session('session-b', 'stopped'),
        { id: '', state: 'active' },
        null,
      ),
      catalog(),
    ];

    await new SessionCleanup({
      list: async () => responses.shift() ?? catalog(),
      kill: async (sessionId) => killed.push(sessionId),
      quietPasses: 1,
      wait: async () => {},
    }).run();

    assert.deepEqual(killed, ['session-a']);
  });

  it('rejects malformed catalogs instead of reporting a clean environment', async () => {
    await assert.rejects(
      new SessionCleanup({
        list: async () => ({}),
        kill: async () => {},
        wait: async () => {},
      }).run(),
      /session catalog response/,
    );
  });

  it('fails when active sessions do not settle before the timeout', async () => {
    let clock = 0;
    await assert.rejects(
      new SessionCleanup({
        list: async () => catalog(session('session-a', 'active')),
        kill: async () => {},
        timeoutMs: 2,
        settleMs: 1,
        now: () => clock,
        wait: async (durationMs) => { clock += durationMs; },
      }).run(),
      /no progress for 2ms/,
    );
  });

  it('renews the timeout after a slow successful kill', async () => {
    let clock = 0;
    const responses = [catalog(session('session-a', 'active')), catalog(), catalog()];

    const result = await new SessionCleanup({
      list: async () => responses.shift() ?? catalog(),
      kill: async () => { clock += 5; },
      timeoutMs: 2,
      settleMs: 1,
      quietPasses: 2,
      now: () => clock,
      wait: async (durationMs) => { clock += durationMs; },
    }).run();

    assert.deepEqual(result.removedSessionIds, ['session-a']);
  });
});

function catalog(...sessions) {
  return { sessions };
}

function session(id, state) {
  return { id, state };
}
