import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  ADMIN_UNIFIED_ROUTE_MANIFEST,
  adminRouteUrl,
  assertEqual,
  assertNoBodyHorizontalOverflow,
  assertNoHorizontalOverflow,
  authJsonHeaders,
  findActiveSessionIdsByLabels,
  waitForContains,
} from './admin-unified-smoke-lib.mjs';

describe('admin unified smoke helpers', () => {
  it('keeps route names, paths, and ready selectors unique', () => {
    assert.equal(ADMIN_UNIFIED_ROUTE_MANIFEST.length, 16);
    assert.equal(new Set(ADMIN_UNIFIED_ROUTE_MANIFEST.map((route) => route.name)).size, 16);
    assert.equal(new Set(ADMIN_UNIFIED_ROUTE_MANIFEST.map((route) => route.path)).size, 16);
    assert.equal(new Set(ADMIN_UNIFIED_ROUTE_MANIFEST.map((route) => route.readyTestId)).size, 16);
  });

  it('builds normalized admin-new URLs and JSON auth headers', () => {
    const options = { pageUrl: 'https://browserpane.test/admin-new/projects' };
    assert.equal(adminRouteUrl(options, '/projects/'), 'https://browserpane.test/admin-new/projects');
    assert.equal(adminRouteUrl(options, ''), 'https://browserpane.test/admin-new/');
    assert.deepEqual(authJsonHeaders('token-1'), {
      Authorization: 'Bearer token-1',
      'content-type': 'application/json',
    });
  });

  it('checks element and document overflow with the same tolerance', async () => {
    await assertNoHorizontalOverflow(fakePage({ clientWidth: 100, scrollWidth: 101 }), 'route', 'route panel');
    await assertNoBodyHorizontalOverflow(fakePage({ clientWidth: 100, scrollWidth: 100 }), 'document');
    await assert.rejects(
      assertNoHorizontalOverflow(fakePage({ clientWidth: 100, scrollWidth: 102 }), 'route', 'route panel'),
      /route panel overflows horizontally/,
    );
  });

  it('waits for visible selector text and reports unequal values', async () => {
    const scope = {
      getByTestId: () => ({ textContent: async () => 'Project saved.' }),
    };
    await waitForContains(scope, { connectTimeoutMs: 10 }, 'project-action-success', 'saved');
    assert.doesNotThrow(() => assertEqual('ready', 'ready', 'state'));
    assert.throws(() => assertEqual('failed', 'ready', 'state'), /Expected state to be "ready"/);
  });

  it('selects only active sessions with all expected smoke labels', () => {
    assert.deepEqual(findActiveSessionIdsByLabels({ sessions: [
      { id: 'active-match', state: 'ready', labels: { suite: 'admin-unified-sessions', purpose: 'smoke' } },
      { id: 'stopped-match', state: 'stopped', labels: { suite: 'admin-unified-sessions', purpose: 'smoke' } },
      { id: 'other-suite', state: 'ready', labels: { suite: 'manual', purpose: 'smoke' } },
      { id: 'missing-label', state: 'ready', labels: { suite: 'admin-unified-sessions' } },
      null,
    ] }, {
      suite: 'admin-unified-sessions',
      purpose: 'smoke',
    }), ['active-match']);
    assert.throws(
      () => findActiveSessionIdsByLabels({}, { suite: 'admin-unified-sessions' }),
      /session catalog response/,
    );
  });
});

function fakePage(size) {
  return {
    getByTestId: () => ({ evaluate: async () => size }),
    evaluate: async () => size,
  };
}
