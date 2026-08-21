import {
  API_AUTH_MODES,
  API_CLASSIFICATIONS,
  API_METHODS,
  COMPATIBILITY_AUTH_MODES,
  COMPATIBILITY_STABILITIES,
  type ApiAuthMode,
  type ApiClassification,
  type ApiClassificationCatalog,
  type ApiContractEvidence,
  type ApiExample,
  type ApiExampleCatalog,
  type ApiMethod,
  type ApiOperation,
  type ApiOperationCatalog,
  type CompatibilityAuthMode,
  type CompatibilityStability,
  type CompatibilitySurface,
  type CompatibilitySurfaceCatalog,
} from './api-contract-types';

const OPERATION_ID_PATTERN = /^[A-Za-z][A-Za-z0-9]*$/;
const COMPATIBILITY_ID_PATTERN = /^[a-z][a-z0-9-]*$/;
const RESPONSE_PATTERN = /^(default|[1-5][0-9]{2})$/;
const SENSITIVE_EXAMPLE_KEY = /^(authorization|access_token|refresh_token|client_secret|password|private_key|cookie)$/i;

type ApiContractClientOptions = {
  readonly baseUrl: string;
  readonly fetchImpl?: typeof fetch;
};

export class ApiContractClient {
  readonly #baseUrl: string;
  readonly #fetch: typeof fetch;

  constructor(options: ApiContractClientOptions) {
    this.#baseUrl = options.baseUrl;
    this.#fetch = options.fetchImpl ?? fetch;
  }

  async load(): Promise<ApiContractEvidence> {
    const [operationsDocument, classificationsDocument, examplesDocument, compatibilityDocument] = await Promise.all([
      this.#getJson('/openapi/bpane-control-v1.operations.json'),
      this.#getJson('/openapi/bpane-control-v1.classifications.json'),
      this.#getJson('/openapi/bpane-control-v1.examples.json'),
      this.#getJson('/openapi/bpane-control-v1.compatibility.json'),
    ]);
    return parseApiContractEvidence(
      operationsDocument,
      classificationsDocument,
      examplesDocument,
      compatibilityDocument,
    );
  }

  async #getJson(path: string): Promise<unknown> {
    const response = await this.#fetch(new URL(path, this.#baseUrl));
    if (!response.ok) {
      throw new Error(`API contract evidence request failed with HTTP ${response.status} for ${path}`);
    }
    try {
      return await response.json();
    } catch {
      throw new Error(`API contract evidence is not valid JSON: ${path}`);
    }
  }
}

export function parseApiContractEvidence(
  operationsInput: unknown,
  classificationsInput: unknown,
  examplesInput: unknown,
  compatibilityInput: unknown,
): ApiContractEvidence {
  const operationCatalog = parseOperationCatalog(operationsInput);
  const classificationCatalog = parseClassificationCatalog(classificationsInput);
  const exampleCatalog = parseExampleCatalog(examplesInput);
  const compatibilityCatalog = parseCompatibilityCatalog(compatibilityInput);

  validateClassifications(operationCatalog.operations, classificationCatalog.classifications);
  validateExamples(operationCatalog.operations, exampleCatalog.examples);
  validateCompatibilityCollisions(operationCatalog.operations, compatibilityCatalog.surfaces);

  return {
    operations: operationCatalog.operations,
    classifications: classificationCatalog.classifications,
    examples: exampleCatalog.examples,
    compatibilitySurfaces: compatibilityCatalog.surfaces,
  };
}

export function parseOperationCatalog(input: unknown): ApiOperationCatalog {
  const document = contractDocument(input, 'operation catalog');
  const rawOperations = requiredArray(document.operations, 'operation catalog operations');
  const ids = new Set<string>();
  const methodPaths = new Set<string>();
  const operations = rawOperations.map((item, index) => {
    const object = requiredObject(item, `operation ${index}`);
    const operationId = requiredPatternString(object.operationId, `operation ${index} id`, OPERATION_ID_PATTERN);
    if (ids.has(operationId)) throw new Error(`Duplicate API operation id: ${operationId}`);
    ids.add(operationId);
    const method = requiredEnum(object.method, API_METHODS, `${operationId} method`);
    const path = requiredPath(object.path, `${operationId} path`, '/api/v1/');
    const methodPath = `${method} ${path}`;
    if (methodPaths.has(methodPath)) throw new Error(`Duplicate API operation method/path: ${methodPath}`);
    methodPaths.add(methodPath);
    const tags = requiredStringArray(object.tags, `${operationId} tags`, false);
    const auth = requiredEnum(object.auth, API_AUTH_MODES, `${operationId} auth`);
    const classification = requiredEnum(object.classification, API_CLASSIFICATIONS, `${operationId} classification`);
    const responses = requiredStringArray(object.responses, `${operationId} responses`, false);
    if (responses.some((response) => !RESPONSE_PATTERN.test(response))) {
      throw new Error(`${operationId} responses contain an unsupported status`);
    }
    return { operationId, method, path, tags, auth, classification, responses } satisfies ApiOperation;
  });
  if (operations.length === 0) throw new Error('API operation catalog must not be empty');
  return { version: 1, contract: 'bpane-control-v1', operations };
}

export function parseClassificationCatalog(input: unknown): ApiClassificationCatalog {
  const document = contractDocument(input, 'classification catalog');
  const classificationsObject = requiredObject(document.classifications, 'classification catalog classifications');
  const keys = Object.keys(classificationsObject);
  if (keys.some((key) => !API_CLASSIFICATIONS.includes(key as ApiClassification)) || keys.length !== API_CLASSIFICATIONS.length) {
    throw new Error('Classification catalog must contain exactly the supported classifications');
  }
  const classifications = Object.fromEntries(API_CLASSIFICATIONS.map((classification) => [
    classification,
    requiredStringArray(classificationsObject[classification], `${classification} operation ids`, true),
  ])) as Record<ApiClassification, readonly string[]>;
  return { version: 1, contract: 'bpane-control-v1', classifications };
}

export function parseExampleCatalog(input: unknown): ApiExampleCatalog {
  const document = contractDocument(input, 'example catalog');
  const rawExamples = requiredArray(document.examples, 'example catalog examples');
  const names = new Set<string>();
  const examples = rawExamples.map((item, index) => {
    const object = requiredObject(item, `example ${index}`);
    const name = requiredPatternString(object.name, `example ${index} name`, COMPATIBILITY_ID_PATTERN);
    if (names.has(name)) throw new Error(`Duplicate API example name: ${name}`);
    names.add(name);
    const operationId = requiredPatternString(object.operationId, `${name} operation id`, OPERATION_ID_PATTERN);
    const requestObject = requiredObject(object.request, `${name} request`);
    const responseObject = requiredObject(object.response, `${name} response`);
    const request = {
      method: requiredEnum(requestObject.method, API_METHODS, `${name} request method`),
      path: requiredPath(requestObject.path, `${name} request path`, '/api/v1/'),
      ...(Object.hasOwn(requestObject, 'headers')
        ? {
            headers: Object.fromEntries(
              Object.entries(requiredObject(requestObject.headers, `${name} request headers`))
                .map(([key, value]) => [key, requiredString(value, `${name} request header ${key}`)]),
            ),
          }
        : {}),
      ...(Object.hasOwn(requestObject, 'body') ? { body: safeExampleValue(requestObject.body, `${name} request body`) } : {}),
    };
    const status = requiredInteger(responseObject.status, `${name} response status`, 100, 599);
    const response = {
      status,
      ...(Object.hasOwn(responseObject, 'body') ? { body: safeExampleValue(responseObject.body, `${name} response body`) } : {}),
    };
    return { name, operationId, request, response } satisfies ApiExample;
  });
  return { version: 1, contract: 'bpane-control-v1', examples };
}

export function parseCompatibilityCatalog(input: unknown): CompatibilitySurfaceCatalog {
  const document = contractDocument(input, 'compatibility catalog');
  const rawSurfaces = requiredArray(document.surfaces, 'compatibility catalog surfaces');
  const ids = new Set<string>();
  const methodPaths = new Set<string>();
  const surfaces = rawSurfaces.map((item, index) => {
    const object = requiredObject(item, `compatibility surface ${index}`);
    const id = requiredPatternString(object.id, `compatibility surface ${index} id`, COMPATIBILITY_ID_PATTERN);
    if (ids.has(id)) throw new Error(`Duplicate compatibility surface id: ${id}`);
    ids.add(id);
    const family = requiredString(object.family, `${id} family`);
    const methods = requiredArray(object.methods, `${id} methods`).map((method) =>
      requiredEnum(method, API_METHODS, `${id} method`));
    if (methods.length === 0 || new Set(methods).size !== methods.length) {
      throw new Error(`${id} methods must be a non-empty unique array`);
    }
    const path = requiredString(object.path, `${id} path`);
    if (!path.startsWith('/') && !path.startsWith('{configured_oidc_issuer}')) {
      throw new Error(`${id} path must be absolute or use the configured OIDC issuer placeholder`);
    }
    for (const method of methods) {
      const methodPath = `${method} ${path}`;
      if (methodPaths.has(methodPath)) throw new Error(`Duplicate compatibility method/path: ${methodPath}`);
      methodPaths.add(methodPath);
    }
    const auth = requiredEnum(object.auth, COMPATIBILITY_AUTH_MODES, `${id} auth`);
    const stability = requiredEnum(object.stability, COMPATIBILITY_STABILITIES, `${id} stability`);
    const purpose = requiredString(object.purpose, `${id} purpose`);
    return { id, family, methods, path, auth, stability, purpose } satisfies CompatibilitySurface;
  });
  return { version: 1, contract: 'bpane-control-v1', surfaces };
}

function validateClassifications(
  operations: readonly ApiOperation[],
  classifications: ApiClassificationCatalog['classifications'],
): void {
  const assignments = new Map<string, ApiClassification>();
  for (const classification of API_CLASSIFICATIONS) {
    for (const operationId of classifications[classification]) {
      if (assignments.has(operationId)) throw new Error(`API operation has multiple classifications: ${operationId}`);
      assignments.set(operationId, classification);
    }
  }
  for (const operation of operations) {
    const assigned = assignments.get(operation.operationId);
    if (!assigned) throw new Error(`API operation is missing from classifications: ${operation.operationId}`);
    if (assigned !== operation.classification) {
      throw new Error(`API operation classification drift: ${operation.operationId}`);
    }
    assignments.delete(operation.operationId);
  }
  if (assignments.size > 0) {
    throw new Error(`Classification catalog contains unknown operations: ${Array.from(assignments.keys()).join(', ')}`);
  }
}

function validateExamples(operations: readonly ApiOperation[], examples: readonly ApiExample[]): void {
  const operationById = new Map(operations.map((operation) => [operation.operationId, operation]));
  for (const example of examples) {
    const operation = operationById.get(example.operationId);
    if (!operation) throw new Error(`API example references an unknown operation: ${example.name}`);
    if (operation.method !== example.request.method || !matchesOperationPath(operation.path, example.request.path)) {
      throw new Error(`API example request does not match operation: ${example.name}`);
    }
    if (!operation.responses.includes(String(example.response.status))) {
      throw new Error(`API example response is not declared by operation: ${example.name}`);
    }
  }
}

function validateCompatibilityCollisions(
  operations: readonly ApiOperation[],
  surfaces: readonly CompatibilitySurface[],
): void {
  const frozen = new Set(operations.map((operation) => `${operation.method} ${stripQuery(operation.path)}`));
  for (const surface of surfaces) {
    for (const method of surface.methods) {
      if (frozen.has(`${method} ${stripQuery(surface.path)}`)) {
        throw new Error(`Compatibility surface collides with frozen API operation: ${surface.id}`);
      }
    }
  }
}

function matchesOperationPath(template: string, concrete: string): boolean {
  const pattern = template
    .split(/(\{[^}]+\})/g)
    .map((part) => part.startsWith('{') ? '[^/?]+' : escapeRegex(part))
    .join('');
  return new RegExp(`^${pattern}(?:\\?.*)?$`).test(concrete);
}

function stripQuery(path: string): string {
  return path.split('?', 1)[0] ?? path;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function contractDocument(input: unknown, label: string): Record<string, unknown> {
  const document = requiredObject(input, label);
  if (document.version !== 1 || document.contract !== 'bpane-control-v1') {
    throw new Error(`${label} must target bpane-control-v1 format version 1`);
  }
  return document;
}

function requiredObject(input: unknown, label: string): Record<string, unknown> {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new Error(`${label} must be an object`);
  return input as Record<string, unknown>;
}

function requiredArray(input: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(input)) throw new Error(`${label} must be an array`);
  return input;
}

function requiredString(input: unknown, label: string): string {
  if (typeof input !== 'string' || input.trim() === '') throw new Error(`${label} must be a non-empty string`);
  return input;
}

function requiredPatternString(input: unknown, label: string, pattern: RegExp): string {
  const value = requiredString(input, label);
  if (!pattern.test(value)) throw new Error(`${label} has an unsupported format`);
  return value;
}

function requiredPath(input: unknown, label: string, prefix: string): string {
  const value = requiredString(input, label);
  if (!value.startsWith(prefix)) throw new Error(`${label} must start with ${prefix}`);
  return value;
}

function requiredEnum<const T extends readonly string[]>(input: unknown, values: T, label: string): T[number] {
  if (typeof input !== 'string' || !values.includes(input)) throw new Error(`${label} is unsupported`);
  return input as T[number];
}

function requiredStringArray(input: unknown, label: string, allowEmpty: boolean): readonly string[] {
  const values = requiredArray(input, label).map((value) => requiredString(value, label));
  if (!allowEmpty && values.length === 0) throw new Error(`${label} must not be empty`);
  if (new Set(values).size !== values.length) throw new Error(`${label} must contain unique values`);
  return values;
}

function requiredInteger(input: unknown, label: string, min: number, max: number): number {
  if (!Number.isInteger(input) || (input as number) < min || (input as number) > max) {
    throw new Error(`${label} must be an integer between ${min} and ${max}`);
  }
  return input as number;
}

function safeExampleValue(input: unknown, label: string): unknown {
  if (Array.isArray(input)) return input.map((value, index) => safeExampleValue(value, `${label}[${index}]`));
  if (input && typeof input === 'object') {
    const output: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(input)) {
      if (SENSITIVE_EXAMPLE_KEY.test(key)) throw new Error(`${label} contains sensitive field ${key}`);
      output[key] = safeExampleValue(value, `${label}.${key}`);
    }
    return output;
  }
  if (input === null || ['string', 'number', 'boolean'].includes(typeof input)) return input;
  throw new Error(`${label} contains an unsupported value`);
}
