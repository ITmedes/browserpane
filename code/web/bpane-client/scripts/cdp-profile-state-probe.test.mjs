import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  isExpectedDocumentReady,
  isTransientDocumentError,
  waitForExpectedDocument,
} from './cdp-profile-state-probe.mjs';

describe('CDP profile state probe document readiness', () => {
  it('accepts the expected ready document and ignores URL fragments', () => {
    assert.equal(isExpectedDocumentReady({
      href: 'http://web:8080/test-embed.html#probe',
      readyState: 'complete',
    }, 'http://web:8080/test-embed.html'), true);
  });

  it('rejects an initial or redirected document on the same origin', () => {
    assert.equal(isExpectedDocumentReady({
      href: 'about:blank',
      readyState: 'complete',
    }, 'http://web:8080/test-embed.html'), false);
    assert.equal(isExpectedDocumentReady({
      href: 'http://web:8080/admin/',
      readyState: 'complete',
    }, 'http://web:8080/test-embed.html'), false);
  });

  it('recognizes CDP errors caused by document replacement', () => {
    assert.equal(isTransientDocumentError(new Error('Inspected target navigated or closed (-32000)')), true);
    assert.equal(isTransientDocumentError(new Error('Execution context was destroyed.')), true);
    assert.equal(isTransientDocumentError(new Error('Permission denied')), false);
  });

  it('waits through navigation churn and requires two stable observations', async () => {
    const results = [
      new Error('Inspected target navigated or closed (-32000)'),
      { href: 'about:blank', readyState: 'complete' },
      { href: 'http://web:8080/test-embed.html', readyState: 'interactive' },
      { href: 'http://web:8080/test-embed.html', readyState: 'complete' },
    ];
    let clock = 0;
    const connection = {
      async send() {
        const result = results.shift();
        if (result instanceof Error) throw result;
        return { result: { value: result } };
      },
    };

    const documentState = await waitForExpectedDocument(
      connection,
      'http://web:8080/test-embed.html',
      1000,
      {
        now: () => clock,
        wait: async (durationMs) => { clock += durationMs; },
      },
    );

    assert.equal(documentState.href, 'http://web:8080/test-embed.html');
    assert.equal(results.length, 0);
  });

  it('fails immediately for non-navigation CDP errors', async () => {
    const connection = {
      async send() {
        throw new Error('Permission denied');
      },
    };

    await assert.rejects(
      waitForExpectedDocument(connection, 'http://web:8080/test-embed.html', 1000),
      /Permission denied/,
    );
  });
});
