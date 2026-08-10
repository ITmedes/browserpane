import fs from 'node:fs';

import { parseDocument } from 'yaml';

const HTTP_METHODS = new Set(['delete', 'get', 'head', 'options', 'patch', 'post', 'put', 'trace']);

export class OpenApiContract {
  static load(filename) {
    return this.parse(fs.readFileSync(filename, 'utf8'));
  }

  static parse(content) {
    const document = parseDocument(content, {
      prettyErrors: true,
      strict: true,
      uniqueKeys: true
    });
    if (document.errors.length > 0) {
      throw new Error(document.errors.map((error) => error.message).join('\n'));
    }
    const value = document.toJS({ maxAliasCount: 100 });
    this.#assertLocalReferences(value);
    return new OpenApiContract(value);
  }

  static #assertLocalReferences(value, location = '$') {
    if (Array.isArray(value)) {
      value.forEach((item, index) => this.#assertLocalReferences(item, `${location}[${index}]`));
      return;
    }
    if (value === null || typeof value !== 'object') return;
    for (const [key, child] of Object.entries(value)) {
      const childLocation = `${location}.${key}`;
      if (key === '$ref') {
        if (typeof child !== 'string' || !/^#(?:\/|$)/.test(child)) {
          throw new Error(`${childLocation} uses an external OpenAPI reference`);
        }
      } else {
        this.#assertLocalReferences(child, childLocation);
      }
    }
  }

  constructor(document) {
    this.document = document;
  }

  operations() {
    const operations = [];
    for (const [routePath, pathItem] of Object.entries(this.document.paths ?? {})) {
      for (const [method, operation] of Object.entries(pathItem ?? {})) {
        if (!HTTP_METHODS.has(method)) continue;
        operations.push(this.#operation(routePath, method, operation));
      }
    }
    return operations.sort((left, right) => String(left.operationId ?? '')
      .localeCompare(String(right.operationId ?? '')));
  }

  #operation(routePath, method, operation) {
    return {
      operationId: operation?.operationId,
      method: method.toUpperCase(),
      path: routePath,
      tags: [...(operation?.tags ?? [])],
      auth: this.#authClass(operation),
      summary: operation?.summary,
      responses: Object.keys(operation?.responses ?? {}).sort()
    };
  }

  #authClass(operation) {
    const security = operation?.security ?? this.document.security ?? [];
    if (security.length === 0) return 'unauthenticated';
    if (security.some((requirement) => 'sessionAutomationAccessToken' in requirement)) {
      return 'session-automation';
    }
    if (security.some((requirement) => 'recordingWorkerAccessToken' in requirement)) {
      return 'recording-worker';
    }
    if (security.some((requirement) => 'bearerAuth' in requirement)) return 'owner-bearer';
    return 'other';
  }
}
