import fs from 'node:fs';

export class ExampleCatalog {
  static load(filename) {
    return this.fromDocument(JSON.parse(fs.readFileSync(filename, 'utf8')));
  }

  static fromDocument(document) {
    if (document.version !== 1 || document.contract !== 'bpane-control-v1') {
      throw new Error('OpenAPI example catalog must target bpane-control-v1 format version 1');
    }
    if (!Array.isArray(document.examples)) {
      throw new Error('OpenAPI example catalog must contain an examples array');
    }
    return document.examples;
  }
}
