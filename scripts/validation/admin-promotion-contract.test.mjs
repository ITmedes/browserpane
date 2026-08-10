import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

import {
  ADMIN_PROMOTION_SMOKES,
  COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
  UNIFIED_ADMIN_PROMOTION_SMOKES,
} from './admin-promotion-contract.mjs';
import { ValidationStageCatalog } from './stage-catalog.mjs';

const root = path.resolve(import.meta.dirname, '../..');
const clientPackage = JSON.parse(fs.readFileSync(
  path.join(root, 'code/web/bpane-client/package.json'),
  'utf8',
));

test('promotion contract includes every unified-admin smoke script', () => {
  const packageScripts = Object.keys(clientPackage.scripts)
    .filter((name) => name.startsWith('smoke:admin-unified-'))
    .sort();
  const promotionScripts = UNIFIED_ADMIN_PROMOTION_SMOKES
    .map((smoke) => smoke.script)
    .sort();

  assert.deepEqual(promotionScripts, packageScripts);
});

test('promotion contract references existing scripts with stable unique ids', () => {
  assert.equal(
    new Set(ADMIN_PROMOTION_SMOKES.map((smoke) => smoke.id)).size,
    ADMIN_PROMOTION_SMOKES.length,
  );
  assert.equal(
    new Set(ADMIN_PROMOTION_SMOKES.map((smoke) => smoke.script)).size,
    ADMIN_PROMOTION_SMOKES.length,
  );
  for (const smoke of ADMIN_PROMOTION_SMOKES) {
    assert.match(smoke.id, /^compose-admin-(?:new|compat|auth)/);
    assert.ok(clientPackage.scripts[smoke.script], `missing package script ${smoke.script}`);
  }
});

test('compatibility contract protects auth, session, and migrated operator anchors', () => {
  const scripts = new Set(COMPATIBILITY_ADMIN_PROMOTION_SMOKES.map((smoke) => smoke.script));

  for (const required of [
    'smoke:admin-auth-security',
    'smoke:admin-session',
    'smoke:admin-realtime',
    'smoke:admin-browser-contexts',
    'smoke:admin-egress-profiles',
    'smoke:admin-session-files',
    'smoke:admin-mcp',
    'smoke:admin-recording',
    'smoke:admin-workflow',
    'smoke:admin-workflow-run-detail',
  ]) {
    assert.ok(scripts.has(required), `missing compatibility promotion smoke ${required}`);
  }
});

test('canonical compose profile contains the complete promotion inventory', () => {
  const composeIds = new Set(
    new ValidationStageCatalog(root).forProfile('compose').map((stage) => stage.id),
  );

  for (const smoke of ADMIN_PROMOTION_SMOKES) {
    assert.ok(composeIds.has(smoke.id), `missing canonical stage ${smoke.id}`);
  }
});
