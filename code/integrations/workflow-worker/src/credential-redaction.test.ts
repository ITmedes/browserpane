import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { CredentialRedactor } from './credential-redaction.js';

describe('credential result redaction', () => {
  it('scrubs resolved payloads and generated TOTP shapes from persisted evidence', () => {
    const redactor = new CredentialRedactor();
    redactor.addPayload({ username: 'demo-user', password: 'secret-value', short: 'pw' });
    redactor.addTotpDigits(6);

    assert.equal(
      redactor.redactText('demo-user secret-value pw 123456 campaign1234567'),
      '[REDACTED] [REDACTED] [REDACTED] [REDACTED] campaign1234567',
    );
    assert.deepEqual(
      redactor.redactValue({ nested: ['secret-value'], safe: true }),
      { nested: ['[REDACTED]'], safe: true },
    );
  });
});
