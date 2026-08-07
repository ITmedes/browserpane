import fs from 'node:fs';

export class ExampleCatalog {
  static load(filename) {
    const document = JSON.parse(fs.readFileSync(filename, 'utf8'));
    if (!Array.isArray(document.examples)) {
      throw new Error('OpenAPI example catalog must contain an examples array');
    }
    return document.examples;
  }
}
