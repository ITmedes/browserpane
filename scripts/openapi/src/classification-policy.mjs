import fs from 'node:fs';

const SUPPORTED_CLASSIFICATIONS = new Set([
  'api-companion',
  'internal-worker',
  'ui-evidence',
  'ui-primary'
]);

export class ClassificationPolicy {
  static load(filename) {
    return new ClassificationPolicy(JSON.parse(fs.readFileSync(filename, 'utf8')));
  }

  constructor(document) {
    this.document = document;
  }

  assignments() {
    if (this.document.version !== 1 || this.document.contract !== 'bpane-control-v1') {
      throw new Error('classification policy must target bpane-control-v1 format version 1');
    }
    const assignments = new Map();
    for (const [classification, operationIds] of Object.entries(
      this.document.classifications ?? {}
    )) {
      if (!SUPPORTED_CLASSIFICATIONS.has(classification)) {
        throw new Error(`unsupported operation classification: ${classification}`);
      }
      if (!Array.isArray(operationIds)) {
        throw new Error(`classification ${classification} must be an array`);
      }
      for (const operationId of operationIds) {
        if (typeof operationId !== 'string' || operationId.length === 0) {
          throw new Error(`classification ${classification} contains an invalid operation id`);
        }
        if (assignments.has(operationId)) {
          throw new Error(`operation ${operationId} has multiple classifications`);
        }
        assignments.set(operationId, classification);
      }
    }
    return assignments;
  }
}
