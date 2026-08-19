export const API_CLASSIFICATIONS = ['ui-primary', 'ui-evidence', 'api-companion', 'internal-worker'] as const;
export const API_AUTH_MODES = [
  'owner-bearer',
  'session-automation',
  'recording-worker',
  'unauthenticated',
] as const;
export const API_METHODS = ['DELETE', 'GET', 'POST', 'PUT'] as const;

export type ApiClassification = (typeof API_CLASSIFICATIONS)[number];
export type ApiAuthMode = (typeof API_AUTH_MODES)[number];
export type ApiMethod = (typeof API_METHODS)[number];

export type ApiOperation = {
  readonly operationId: string;
  readonly method: ApiMethod;
  readonly path: string;
  readonly tags: readonly string[];
  readonly auth: ApiAuthMode;
  readonly classification: ApiClassification;
  readonly responses: readonly string[];
};

export type ApiOperationCatalog = {
  readonly version: 1;
  readonly contract: 'bpane-control-v1';
  readonly operations: readonly ApiOperation[];
};

export type ApiClassificationCatalog = {
  readonly version: 1;
  readonly contract: 'bpane-control-v1';
  readonly classifications: Readonly<Record<ApiClassification, readonly string[]>>;
};

export type ApiExampleRequest = {
  readonly method: ApiMethod;
  readonly path: string;
  readonly body?: unknown;
};

export type ApiExampleResponse = {
  readonly status: number;
  readonly body?: unknown;
};

export type ApiExample = {
  readonly name: string;
  readonly operationId: string;
  readonly request: ApiExampleRequest;
  readonly response: ApiExampleResponse;
};

export type ApiExampleCatalog = {
  readonly version: 1;
  readonly contract: 'bpane-control-v1';
  readonly examples: readonly ApiExample[];
};

export const COMPATIBILITY_AUTH_MODES = [
  'deployment-internal',
  'internal-bearer',
  'mcp-protocol',
  'owner-bearer',
  'public-metadata',
] as const;

export const COMPATIBILITY_STABILITIES = [
  'compatibility',
  'deployment-helper',
  'development-helper',
  'external-dependency',
  'legacy',
  'legacy-protocol',
  'protocol',
] as const;

export type CompatibilityAuthMode = (typeof COMPATIBILITY_AUTH_MODES)[number];
export type CompatibilityStability = (typeof COMPATIBILITY_STABILITIES)[number];

export type CompatibilitySurface = {
  readonly id: string;
  readonly family: string;
  readonly methods: readonly ApiMethod[];
  readonly path: string;
  readonly auth: CompatibilityAuthMode;
  readonly stability: CompatibilityStability;
  readonly purpose: string;
};

export type CompatibilitySurfaceCatalog = {
  readonly version: 1;
  readonly contract: 'bpane-control-v1';
  readonly surfaces: readonly CompatibilitySurface[];
};

export type ApiContractEvidence = {
  readonly operations: readonly ApiOperation[];
  readonly classifications: ApiClassificationCatalog['classifications'];
  readonly examples: readonly ApiExample[];
  readonly compatibilitySurfaces: readonly CompatibilitySurface[];
};

export type ApiContractLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly evidence: ApiContractEvidence }
  | { readonly status: 'error'; readonly message: string };
