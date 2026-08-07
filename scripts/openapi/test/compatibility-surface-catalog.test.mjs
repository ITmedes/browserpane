import assert from 'node:assert/strict';
import test from 'node:test';

import { CompatibilitySurfaceCatalog } from '../src/compatibility-surface-catalog.mjs';

const DOCUMENT = {
  version: 1,
  contract: 'bpane-control-v1',
  surfaces: [{
    id: 'legacy-status',
    family: 'Gateway legacy',
    methods: ['GET'],
    path: '/api/session/status',
    auth: 'owner-bearer',
    stability: 'legacy',
    purpose: 'Legacy status.'
  }]
};

test('accepts valid non-v1 compatibility surfaces', () => {
  const surfaces = CompatibilitySurfaceCatalog.fromDocument(DOCUMENT);
  assert.equal(surfaces.length, 1);
  assert.doesNotThrow(() => CompatibilitySurfaceCatalog.validateAgainstInventory(
    surfaces,
    [{ method: 'GET', path: '/api/v1/sessions' }]
  ));
});

test('rejects unsupported versions and malformed collections', () => {
  assert.throws(
    () => CompatibilitySurfaceCatalog.fromDocument({ ...DOCUMENT, version: 2 }),
    /format version 1/
  );
  assert.throws(
    () => CompatibilitySurfaceCatalog.fromDocument({ ...DOCUMENT, surfaces: null }),
    /surfaces array/
  );
});

test('rejects duplicate ids and method paths', () => {
  assert.throws(
    () => CompatibilitySurfaceCatalog.fromDocument({
      ...DOCUMENT,
      surfaces: [DOCUMENT.surfaces[0], { ...DOCUMENT.surfaces[0] }]
    }),
    /Duplicate compatibility surface id/
  );
  assert.throws(
    () => CompatibilitySurfaceCatalog.fromDocument({
      ...DOCUMENT,
      surfaces: [
        DOCUMENT.surfaces[0],
        { ...DOCUMENT.surfaces[0], id: 'legacy-status-alias' }
      ]
    }),
    /Duplicate compatibility method\/path/
  );
});

test('rejects malformed paths, methods, auth, and stability', () => {
  for (const [field, value, pattern] of [
    ['path', 'relative', /path must be absolute/],
    ['methods', ['TRACE'], /unsupported method/],
    ['auth', 'browser-token', /unsupported auth/],
    ['stability', 'stable-v1', /unsupported stability/]
  ]) {
    assert.throws(
      () => CompatibilitySurfaceCatalog.fromDocument({
        ...DOCUMENT,
        surfaces: [{ ...DOCUMENT.surfaces[0], [field]: value }]
      }),
      pattern
    );
  }
});

test('rejects collisions with frozen OpenAPI operations', () => {
  assert.throws(
    () => CompatibilitySurfaceCatalog.validateAgainstInventory(
      CompatibilitySurfaceCatalog.fromDocument(DOCUMENT),
      [{ method: 'GET', path: '/api/session/status' }]
    ),
    /collide with frozen operations/
  );
});
