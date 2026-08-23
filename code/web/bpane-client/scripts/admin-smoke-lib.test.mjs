import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { cleanupAdminSessionIds } from './admin-smoke-lib.mjs';

const originalFetch = globalThis.fetch;

afterEach(() => {
  globalThis.fetch = originalFetch;
});

describe('admin smoke cleanup helpers', () => {
  it('releases each explicitly borrowed session once', async () => {
    const requests = [];
    globalThis.fetch = async (url, init) => {
      requests.push({ url: String(url), init });
      return new Response(null, { status: 204 });
    };

    await cleanupAdminSessionIds(
      'owner-token',
      { pageUrl: 'https://browserpane.test/admin/' },
      ['session-1', '', 'session-1', 'session-2'],
    );

    assert.deepEqual(requests.map((request) => request.url), [
      'https://browserpane.test/api/v1/sessions/session-1/kill',
      'https://browserpane.test/api/v1/sessions/session-2/kill',
    ]);
    assert.ok(requests.every((request) => request.init.method === 'POST'));
    assert.ok(requests.every((request) => request.init.headers.Authorization === 'Bearer owner-token'));
  });

  it('surfaces an explicit cleanup failure', async () => {
    globalThis.fetch = async () => new Response('runtime cleanup failed', { status: 500 });

    await assert.rejects(
      cleanupAdminSessionIds(
        'owner-token',
        { pageUrl: 'https://browserpane.test/admin/' },
        ['session-1'],
      ),
      /HTTP 500 runtime cleanup failed/,
    );
  });
});
