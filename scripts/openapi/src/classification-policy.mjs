import fs from 'node:fs';

export class ClassificationPolicy {
  static load(filename) {
    return new ClassificationPolicy(JSON.parse(fs.readFileSync(filename, 'utf8')));
  }

  constructor(document) {
    this.document = document;
  }

  assignments() {
    const assignments = new Map();
    for (const [classification, operationIds] of Object.entries(
      this.document.classifications ?? {}
    )) {
      for (const operationId of operationIds) {
        if (assignments.has(operationId)) {
          throw new Error(`operation ${operationId} has multiple classifications`);
        }
        assignments.set(operationId, classification);
      }
    }
    return assignments;
  }
}
