import assert from 'node:assert/strict';
import test from 'node:test';

import { ContractInventory } from '../src/contract-inventory.mjs';

class ContractStub {
  constructor(operations) {
    this.values = operations;
  }

  operations() {
    return this.values;
  }
}

class PolicyStub {
  constructor(assignments) {
    this.values = new Map(assignments);
  }

  assignments() {
    return this.values;
  }
}

function operation(operationId, overrides = {}) {
  return {
    operationId,
    method: 'GET',
    path: `/api/v1/${operationId}`,
    tags: ['Tests'],
    auth: 'owner-bearer',
    summary: 'Test operation',
    responses: ['200', '401'],
    ...overrides
  };
}

test('joins contract operations with exactly one classification', () => {
  const inventory = new ContractInventory(
    new ContractStub([operation('listWidgets')]),
    new PolicyStub([['listWidgets', 'ui-primary']])
  ).build();

  assert.equal(inventory.operations.length, 1);
  assert.equal(inventory.operations[0].classification, 'ui-primary');
});

test('rejects missing and stale classifications together', () => {
  assert.throws(() => new ContractInventory(
    new ContractStub([operation('listWidgets')]),
    new PolicyStub([['oldOperation', 'api-companion']])
  ).build(), (error) => {
    assert.match(error.message, /operation listWidgets has no classification/);
    assert.match(error.message, /unknown operation: oldOperation/);
    return true;
  });
});

test('rejects duplicate ids, missing tags, and unknown security classes', () => {
  assert.throws(() => new ContractInventory(
    new ContractStub([
      operation('duplicate'),
      operation('duplicate', { tags: [], auth: 'other' })
    ]),
    new PolicyStub([['duplicate', 'internal-worker']])
  ).build(), /duplicate operationId[\s\S]*unsupported security class[\s\S]*has no tag/);
});

test('reports a missing operation id without failing during ordering', () => {
  assert.throws(() => new ContractInventory(
    new ContractStub([operation(undefined)]),
    new PolicyStub([])
  ).build(), /has no operationId/);
});
