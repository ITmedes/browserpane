import { describe, expect, it } from 'vitest';
import { BpaneSession as PublicBpaneSession } from '../bpane.js';
import { BpaneSession as InternalBpaneSession } from '../session/bpane-session.js';

describe('bpane facade', () => {
  it('re-exports the session implementation from the public module', () => {
    expect(PublicBpaneSession).toBe(InternalBpaneSession);
  });
});
