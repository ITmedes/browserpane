import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { resolveNewSessionId } from './new-session-selection.mjs';

describe('resolveNewSessionId', () => {
  it('returns the one session absent from the pre-create catalog', () => {
    assert.equal(
      resolveNewSessionId(['session-a', 'session-b'], ['session-new', 'session-a', 'session-b']),
      'session-new',
    );
  });

  it('returns an empty value while the catalog has not changed', () => {
    assert.equal(resolveNewSessionId(['session-a'], ['session-a']), '');
  });

  it('ignores empty and duplicate catalog entries', () => {
    assert.equal(resolveNewSessionId(['session-a'], ['', 'session-new', 'session-new', 'session-a']), 'session-new');
  });

  it('rejects an ambiguous create result with actionable session ids', () => {
    assert.throws(
      () => resolveNewSessionId(['session-a'], ['session-a', 'session-b', 'session-c']),
      /Expected exactly one newly created session, observed 2: session-b, session-c/,
    );
  });

  it('bounds ambiguous catalog diagnostics', () => {
    assert.throws(
      () => resolveNewSessionId([], ['a', 'b', 'c', 'd', 'e', 'f', 'g']),
      /observed 7: a, b, c, d, e \(\+2 more\)/,
    );
  });
});
