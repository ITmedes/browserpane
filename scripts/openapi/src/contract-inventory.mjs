export class ContractInventory {
  constructor(contract, classificationPolicy) {
    this.contract = contract;
    this.classificationPolicy = classificationPolicy;
  }

  build() {
    const operations = this.contract.operations();
    const classifications = this.classificationPolicy.assignments();
    const errors = this.#validate(operations, classifications);
    if (errors.length > 0) throw new Error(errors.join('\n'));
    return {
      version: 1,
      contract: 'bpane-control-v1',
      operations: operations.map((operation) => ({
        operationId: operation.operationId,
        method: operation.method,
        path: operation.path,
        tags: operation.tags,
        auth: operation.auth,
        classification: classifications.get(operation.operationId),
        responses: operation.responses
      }))
    };
  }

  #validate(operations, classifications) {
    const errors = [];
    const operationIds = new Set();
    for (const operation of operations) {
      if (typeof operation.operationId !== 'string' || operation.operationId.length === 0) {
        errors.push(`${operation.method} ${operation.path} has no operationId`);
        continue;
      }
      if (operationIds.has(operation.operationId)) {
        errors.push(`duplicate operationId: ${operation.operationId}`);
      }
      operationIds.add(operation.operationId);
      if (!classifications.has(operation.operationId)) {
        errors.push(`operation ${operation.operationId} has no classification`);
      }
      if (operation.tags.length === 0) errors.push(`operation ${operation.operationId} has no tag`);
      if (operation.auth === 'other') {
        errors.push(`operation ${operation.operationId} has an unsupported security class`);
      }
    }
    for (const operationId of classifications.keys()) {
      if (!operationIds.has(operationId)) {
        errors.push(`classification references unknown operation: ${operationId}`);
      }
    }
    return errors.sort();
  }
}
