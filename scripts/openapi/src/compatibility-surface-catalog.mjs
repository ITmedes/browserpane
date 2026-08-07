import fs from 'node:fs';

const METHOD_PATTERN = /^(DELETE|GET|PATCH|POST|PUT)$/;
const ID_PATTERN = /^[a-z][a-z0-9-]*$/;
const ALLOWED_AUTH = new Set([
  'deployment-internal',
  'internal-bearer',
  'mcp-protocol',
  'owner-bearer',
  'public-metadata'
]);
const ALLOWED_STABILITY = new Set([
  'compatibility',
  'deployment-helper',
  'development-helper',
  'external-dependency',
  'legacy',
  'legacy-protocol',
  'protocol'
]);

export class CompatibilitySurfaceCatalog {
  static load(filename) {
    return this.fromDocument(JSON.parse(fs.readFileSync(filename, 'utf8')));
  }

  static fromDocument(document) {
    if (document.version !== 1 || document.contract !== 'bpane-control-v1') {
      throw new Error('Compatibility catalog must target bpane-control-v1 format version 1');
    }
    if (!Array.isArray(document.surfaces)) {
      throw new Error('Compatibility catalog must contain a surfaces array');
    }

    const ids = new Set();
    const methodPaths = new Set();
    for (const surface of document.surfaces) {
      this.#validateSurface(surface, ids, methodPaths);
    }
    return document.surfaces;
  }

  static validateAgainstInventory(surfaces, operations) {
    const frozen = new Set(operations.map((operation) =>
      `${operation.method} ${stripQuery(operation.path)}`));
    const collisions = [];
    for (const surface of surfaces) {
      for (const method of surface.methods) {
        const key = `${method} ${stripQuery(surface.path)}`;
        if (frozen.has(key)) collisions.push(`${surface.id}: ${key}`);
      }
    }
    if (collisions.length > 0) {
      throw new Error(`Compatibility surfaces collide with frozen operations:\n${collisions.join('\n')}`);
    }
  }

  static #validateSurface(surface, ids, methodPaths) {
    if (!surface || typeof surface !== 'object' || Array.isArray(surface)) {
      throw new Error('Compatibility surface entries must be objects');
    }
    if (typeof surface.id !== 'string' || !ID_PATTERN.test(surface.id)) {
      throw new Error('Compatibility surface id must use lowercase kebab-case');
    }
    if (ids.has(surface.id)) throw new Error(`Duplicate compatibility surface id: ${surface.id}`);
    ids.add(surface.id);

    for (const field of ['family', 'path', 'purpose']) {
      if (typeof surface[field] !== 'string' || surface[field].trim() === '') {
        throw new Error(`${surface.id}: ${field} must be a non-empty string`);
      }
    }
    if (!surface.path.startsWith('/') && !surface.path.startsWith('{configured_oidc_issuer}')) {
      throw new Error(`${surface.id}: path must be absolute or use the configured OIDC issuer placeholder`);
    }
    if (!Array.isArray(surface.methods) || surface.methods.length === 0) {
      throw new Error(`${surface.id}: methods must be a non-empty array`);
    }
    for (const method of surface.methods) {
      if (typeof method !== 'string' || !METHOD_PATTERN.test(method)) {
        throw new Error(`${surface.id}: unsupported method ${String(method)}`);
      }
      const methodPath = `${method} ${surface.path}`;
      if (methodPaths.has(methodPath)) {
        throw new Error(`Duplicate compatibility method/path: ${methodPath}`);
      }
      methodPaths.add(methodPath);
    }
    if (!ALLOWED_AUTH.has(surface.auth)) {
      throw new Error(`${surface.id}: unsupported auth ${String(surface.auth)}`);
    }
    if (!ALLOWED_STABILITY.has(surface.stability)) {
      throw new Error(`${surface.id}: unsupported stability ${String(surface.stability)}`);
    }
  }
}

function stripQuery(path) {
  return path.split('?', 1)[0];
}
