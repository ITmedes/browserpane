import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ADMIN_PROMOTION_SMOKES,
  COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
  UNIFIED_ADMIN_PROMOTION_SMOKES,
} from './admin-promotion-contract.mjs';
import { selectPromotionSmokes } from '../run-admin-promotion-validation.mjs';

test('promotion runner selects the requested isolated surface', () => {
  assert.equal(selectPromotionSmokes('all'), ADMIN_PROMOTION_SMOKES);
  assert.equal(selectPromotionSmokes('unified'), UNIFIED_ADMIN_PROMOTION_SMOKES);
  assert.equal(
    selectPromotionSmokes('compatibility'),
    COMPATIBILITY_ADMIN_PROMOTION_SMOKES,
  );
});

test('promotion runner rejects unknown surfaces', () => {
  assert.throws(() => selectPromotionSmokes('legacy'), /unknown admin promotion surface/);
});
