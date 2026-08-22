import assert from 'node:assert/strict';
import test from 'node:test';

import {
  isSessionApplicationReady,
  isSessionFileTransferReady,
} from './session-readiness.mjs';

test('transport connection alone is not application readiness', () => {
  assert.equal(isSessionApplicationReady({
    connected: true,
    applicationReady: false,
    sessionId: 'session-1',
  }, 'session-1'), false);
});

test('application readiness is bound to the expected session', () => {
  const state = {
    connected: true,
    applicationReady: true,
    sessionId: 'session-1',
  };

  assert.equal(isSessionApplicationReady(state, 'session-1'), true);
  assert.equal(isSessionApplicationReady(state, 'session-2'), false);
});

test('file transfer readiness requires the advertised capability', () => {
  const state = {
    connected: true,
    applicationReady: true,
    sessionId: 'session-1',
    capabilities: { fileTransfer: false },
  };

  assert.equal(isSessionFileTransferReady(state, 'session-1'), false);
  assert.equal(isSessionFileTransferReady({
    ...state,
    capabilities: { fileTransfer: true },
  }, 'session-1'), true);
});
