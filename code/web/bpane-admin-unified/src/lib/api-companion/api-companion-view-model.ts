import {
  API_AUTH_MODES,
  API_CLASSIFICATIONS,
  type ApiAuthMode,
  type ApiClassification,
  type ApiContractEvidence,
  type ApiExample,
  type ApiOperation,
  type CompatibilitySurface,
} from './api-contract-types';

export type ApiClassificationDefinition = {
  readonly id: ApiClassification;
  readonly label: string;
  readonly shortLabel: string;
  readonly description: string;
};

export type ApiAuthDefinition = {
  readonly id: ApiAuthMode;
  readonly label: string;
  readonly description: string;
};

export type ApiSummaryItem = {
  readonly id: string;
  readonly label: string;
  readonly count: number;
};

export type ApiTaskStep = {
  readonly id: string;
  readonly title: string;
  readonly operation: ApiOperation;
  readonly example: ApiExample;
  readonly command: string;
  readonly coverageHref: string;
};

export type ApiTaskFlow = {
  readonly id: string;
  readonly title: string;
  readonly description: string;
  readonly adminHref: string;
  readonly steps: readonly ApiTaskStep[];
};

export type ApiCoverageFilters = {
  readonly query: string;
  readonly classification: ApiClassification | 'all';
  readonly auth: ApiAuthMode | 'all';
  readonly family: string | 'all';
};

export const CLASSIFICATION_DEFINITIONS: readonly ApiClassificationDefinition[] = [
  {
    id: 'ui-primary',
    label: 'First-class operator UI',
    shortLabel: 'UI primary',
    description: 'Normal operator workflows exposed through a dedicated admin-new control.',
  },
  {
    id: 'ui-evidence',
    label: 'Operator evidence',
    shortLabel: 'UI evidence',
    description: 'Read-only or supporting evidence presented inside an existing operator workflow.',
  },
  {
    id: 'api-companion',
    label: 'API companion',
    shortLabel: 'API companion',
    description: 'Public owner operation documented for integrators without a dedicated UI command.',
  },
  {
    id: 'internal-worker',
    label: 'Internal worker',
    shortLabel: 'Worker only',
    description: 'Worker, recorder, bridge, or observer operation that is not a normal operator action.',
  },
];

export const AUTH_DEFINITIONS: readonly ApiAuthDefinition[] = [
  {
    id: 'owner-bearer',
    label: 'Owner bearer',
    description: 'OIDC access for owner-scoped control-plane resources.',
  },
  {
    id: 'session-automation',
    label: 'Session automation',
    description: 'Short-lived session/run worker credential with a narrower purpose.',
  },
  {
    id: 'unauthenticated',
    label: 'Protocol bootstrap',
    description: 'A credential-free initial transport step with explicit follow-up authentication.',
  },
];

const TASK_FLOW_CONFIG = [
  {
    id: 'projects',
    title: 'Create a project boundary',
    description: 'Create the policy and quota boundary that owns sessions, workflows, and files.',
    adminHref: '/admin-new/projects',
    examples: [{ name: 'companion-project-create', title: 'Create project' }],
  },
  {
    id: 'sessions',
    title: 'Create and connect a session',
    description: 'Persist a browser session, then mint the short-lived ticket consumed by the browser transport.',
    adminHref: '/admin-new/sessions',
    examples: [
      { name: 'companion-session-create', title: 'Create session' },
      { name: 'companion-session-connect-ticket', title: 'Mint connect ticket' },
    ],
  },
  {
    id: 'workflows',
    title: 'Invoke a workflow run',
    description: 'Launch one immutable workflow version with source-system and idempotency correlation.',
    adminHref: '/admin-new/workflows',
    examples: [
      { name: 'workflow-definitions-empty-list', title: 'List workflow definitions' },
      { name: 'companion-workflow-run-create', title: 'Create workflow run' },
    ],
  },
  {
    id: 'file-workspaces',
    title: 'Create a file workspace',
    description: 'Create a governed boundary for reusable workflow inputs and produced files.',
    adminHref: '/admin-new/files/workspaces',
    examples: [
      { name: 'file-workspaces-empty-list', title: 'List file workspaces' },
      { name: 'companion-file-workspace-create', title: 'Create file workspace' },
    ],
  },
] as const;

const PLACEHOLDER_IDS: Readonly<Record<string, string>> = {
  '11111111-1111-4111-8111-111111111111': '${BPANE_PROJECT_ID}',
  '22222222-2222-4222-8222-222222222222': '${BPANE_SESSION_ID}',
  '33333333-3333-4333-8333-333333333333': '${BPANE_WORKFLOW_ID}',
};

export function buildApiTaskFlows(evidence: ApiContractEvidence): readonly ApiTaskFlow[] {
  const operationById = new Map(evidence.operations.map((operation) => [operation.operationId, operation]));
  const exampleByName = new Map(evidence.examples.map((example) => [example.name, example]));
  return TASK_FLOW_CONFIG.map((flow) => ({
    id: flow.id,
    title: flow.title,
    description: flow.description,
    adminHref: flow.adminHref,
    steps: flow.examples.map((configuredStep) => {
      const example = exampleByName.get(configuredStep.name);
      if (!example) throw new Error(`Required API companion example is missing: ${configuredStep.name}`);
      const operation = operationById.get(example.operationId);
      if (!operation) throw new Error(`API companion example references an unknown operation: ${configuredStep.name}`);
      return {
        id: configuredStep.name,
        title: configuredStep.title,
        operation,
        example,
        command: commandForExample(operation, example),
        coverageHref: `/admin-new/coverage?operation=${encodeURIComponent(operation.operationId)}`,
      };
    }),
  }));
}

export function commandForExample(operation: ApiOperation, example: ApiExample): string {
  const path = replacePlaceholders(example.request.path);
  const lines = [
    'curl --fail-with-body \\',
    `  --request ${operation.method} \\`,
    `  "\${BPANE_BASE_URL}${path}" \\`,
  ];
  if (operation.auth === 'owner-bearer') {
    lines.push('  --header "Authorization: Bearer ${BPANE_OWNER_TOKEN}" \\');
  } else if (operation.auth === 'session-automation') {
    lines.push('  --header "Authorization: Bearer ${BPANE_SESSION_AUTOMATION_TOKEN}" \\');
  }
  if (example.request.body !== undefined) {
    lines.push('  --header "Content-Type: application/json" \\');
    lines.push('  --data @- <<JSON');
    lines.push(replacePlaceholders(JSON.stringify(example.request.body, null, 2)));
    lines.push('JSON');
  } else {
    const last = lines.at(-1);
    if (last) lines[lines.length - 1] = last.replace(/ \\$/, '');
  }
  return lines.join('\n');
}

export function operationFamilies(operations: readonly ApiOperation[]): readonly string[] {
  return Array.from(new Set(operations.flatMap((operation) => operation.tags))).sort(localeCompare);
}

export function classificationSummaries(operations: readonly ApiOperation[]): readonly ApiSummaryItem[] {
  return CLASSIFICATION_DEFINITIONS.map((definition) => ({
    id: definition.id,
    label: definition.shortLabel,
    count: operations.filter((operation) => operation.classification === definition.id).length,
  }));
}

export function authSummaries(operations: readonly ApiOperation[]): readonly ApiSummaryItem[] {
  return AUTH_DEFINITIONS.map((definition) => ({
    id: definition.id,
    label: definition.label,
    count: operations.filter((operation) => operation.auth === definition.id).length,
  }));
}

export function filterApiOperations(
  operations: readonly ApiOperation[],
  filters: ApiCoverageFilters,
): readonly ApiOperation[] {
  const query = filters.query.trim().toLocaleLowerCase();
  return operations
    .filter((operation) => filters.classification === 'all' || operation.classification === filters.classification)
    .filter((operation) => filters.auth === 'all' || operation.auth === filters.auth)
    .filter((operation) => filters.family === 'all' || operation.tags.includes(filters.family))
    .filter((operation) => !query || [
      operation.operationId,
      operation.method,
      operation.path,
      operation.auth,
      operation.classification,
      ...operation.tags,
      ...operation.responses,
    ].some((value) => value.toLocaleLowerCase().includes(query)))
    .toSorted((left, right) => localeCompare(left.tags[0] ?? '', right.tags[0] ?? '')
      || localeCompare(left.path, right.path)
      || localeCompare(left.method, right.method));
}

export function groupCompatibilitySurfaces(
  surfaces: readonly CompatibilitySurface[],
): readonly { readonly family: string; readonly surfaces: readonly CompatibilitySurface[] }[] {
  const groups = new Map<string, CompatibilitySurface[]>();
  for (const surface of surfaces) {
    const entries = groups.get(surface.family) ?? [];
    entries.push(surface);
    groups.set(surface.family, entries);
  }
  return Array.from(groups.entries())
    .map(([family, entries]) => ({
      family,
      surfaces: entries.toSorted((left, right) => localeCompare(left.path, right.path)),
    }))
    .toSorted((left, right) => localeCompare(left.family, right.family));
}

export function classificationDefinition(id: ApiClassification): ApiClassificationDefinition {
  return CLASSIFICATION_DEFINITIONS.find((definition) => definition.id === id)
    ?? unreachable(`Unknown classification: ${id}`);
}

export function authDefinition(id: ApiAuthMode): ApiAuthDefinition {
  return AUTH_DEFINITIONS.find((definition) => definition.id === id)
    ?? unreachable(`Unknown auth mode: ${id}`);
}

function replacePlaceholders(value: string): string {
  return Object.entries(PLACEHOLDER_IDS).reduce(
    (result, [placeholder, variable]) => result.replaceAll(placeholder, variable),
    value,
  );
}

function localeCompare(left: string, right: string): number {
  return left.localeCompare(right, 'en');
}

function unreachable(message: string): never {
  throw new Error(message);
}
