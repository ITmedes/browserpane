import {
  formatDateTime,
  type ProjectTone,
} from '$lib/projects/project-formatters';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
} from './workflow-types';
import {
  isAdminTemplateDefinition,
  workflowDefinitionKind,
} from './workflow-visibility';

export type WorkflowVersionMap = Readonly<Record<string, WorkflowDefinitionVersionResource | null>>;

export type WorkflowOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly definitions: readonly WorkflowDefinitionResource[];
      readonly versions: WorkflowVersionMap;
      readonly hiddenCount: number;
      readonly includeHidden: boolean;
    };

export type WorkflowOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type WorkflowOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly kind: string;
  readonly kindTone: ProjectTone;
  readonly latestVersion: string;
  readonly latestTone: ProjectTone;
  readonly executor: string;
  readonly source: string;
  readonly sourceCommit: string;
  readonly labels: string;
  readonly updatedAt: string;
  readonly badges: readonly string[];
};

export type WorkflowOverviewModel = {
  readonly metrics: readonly WorkflowOverviewMetric[];
  readonly rows: readonly WorkflowOverviewRow[];
  readonly hiddenCount: number;
};

export type MetadataRow = {
  readonly label: string;
  readonly value: string;
};

export type WorkflowVersionRow = {
  readonly id: string;
  readonly version: string;
  readonly executor: string;
  readonly createdAt: string;
  readonly latest: boolean;
};

export type WorkflowVersionDetail = {
  readonly id: string;
  readonly version: string;
  readonly executor: string;
  readonly entrypoint: string;
  readonly sourceRows: readonly MetadataRow[];
  readonly policyRows: readonly MetadataRow[];
  readonly schemaRows: readonly MetadataRow[];
  readonly packageRows: readonly MetadataRow[];
  readonly compatibilityRows: readonly MetadataRow[];
};

export type WorkflowDefinitionDetailModel = {
  readonly definitionId: string;
  readonly name: string;
  readonly description: string;
  readonly kind: string;
  readonly latestVersion: string;
  readonly labels: readonly MetadataRow[];
  readonly versionRows: readonly WorkflowVersionRow[];
  readonly selectedVersion: WorkflowVersionDetail | null;
};

export function buildWorkflowOverviewModel(
  definitions: readonly WorkflowDefinitionResource[],
  versions: WorkflowVersionMap = {},
  hiddenCount = 0,
): WorkflowOverviewModel {
  return {
    hiddenCount,
    metrics: [
      metric('visible', 'Visible definitions', definitions.length),
      metric('templates', 'Templates', definitions.filter(isAdminTemplateDefinition).length),
      metric('published', 'Published', definitions.filter((definition) => Boolean(definition.latest_version)).length),
      metric('hidden', 'Hidden internal', hiddenCount),
    ],
    rows: definitions.map((definition) => workflowOverviewRow(definition, versions[definition.id] ?? null)),
  };
}

export function workflowOverviewRow(
  definition: WorkflowDefinitionResource,
  latestVersion: WorkflowDefinitionVersionResource | null = null,
): WorkflowOverviewRow {
  const hasPublishedVersion = Boolean(definition.latest_version);
  const kind = workflowDefinitionKind(definition);
  return {
    id: definition.id,
    name: definition.name,
    description: definition.description?.trim() || 'No description available.',
    kind,
    kindTone: kind === 'Example template' ? 'success' : isAdminTemplateDefinition(definition) ? 'warning' : 'neutral',
    latestVersion: definition.latest_version ?? 'No version',
    latestTone: hasPublishedVersion ? 'success' : 'warning',
    executor: latestVersion?.executor ?? (hasPublishedVersion ? 'Version metadata unavailable' : 'No published version'),
    source: sourceSummary(latestVersion),
    sourceCommit: latestVersion?.source?.resolved_commit ?? 'Not pinned',
    labels: labelSummary(definition.labels),
    updatedAt: formatDateTime(definition.updated_at),
    badges: [
      kind,
      definition.latest_version ?? 'No version',
      latestVersion?.executor ?? 'No executor',
      sourceSummary(latestVersion),
      ...Object.entries(definition.labels).map(([key, value]) => `${key}=${value}`),
    ],
  };
}

export function workflowMatchesSearch(row: WorkflowOverviewRow, normalizedQuery: string): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.name,
    row.description,
    row.kind,
    row.latestVersion,
    row.executor,
    row.source,
    row.sourceCommit,
    row.labels,
    row.updatedAt,
    ...row.badges,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function buildWorkflowDefinitionDetailModel(input: {
  readonly definition: WorkflowDefinitionResource;
  readonly versions: readonly WorkflowDefinitionVersionResource[];
  readonly selectedVersion: string;
}): WorkflowDefinitionDetailModel {
  const selected = input.versions.find((version) => version.version === input.selectedVersion)
    ?? input.versions.find((version) => version.version === input.definition.latest_version)
    ?? input.versions[0]
    ?? null;
  return {
    definitionId: input.definition.id,
    name: input.definition.name,
    description: input.definition.description ?? 'No description available.',
    kind: workflowDefinitionKind(input.definition),
    latestVersion: input.definition.latest_version ?? 'No published version',
    labels: labelRows(input.definition.labels),
    versionRows: input.versions.map((version) => ({
      id: version.id,
      version: version.version,
      executor: version.executor,
      createdAt: formatDateTime(version.created_at),
      latest: version.version === input.definition.latest_version,
    })),
    selectedVersion: selected ? versionDetail(selected) : null,
  };
}

export function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

export function summarizeJson(value: unknown): string {
  if (value === undefined || value === null) {
    return 'Unavailable';
  }
  const serialized = JSON.stringify(value);
  if (!serialized) {
    return 'Unavailable';
  }
  return serialized.length > 180 ? `${serialized.slice(0, 180)}...` : serialized;
}

function versionDetail(version: WorkflowDefinitionVersionResource): WorkflowVersionDetail {
  return {
    id: version.id,
    version: version.version,
    executor: version.executor,
    entrypoint: version.entrypoint,
    sourceRows: [
      { label: 'Kind', value: version.source?.kind ?? 'No source' },
      { label: 'Repository', value: version.source?.repository_url ?? 'Unavailable' },
      { label: 'Ref', value: version.source?.ref ?? 'Unavailable' },
      { label: 'Resolved commit', value: version.source?.resolved_commit ?? 'Unavailable' },
      { label: 'Root path', value: version.source?.root_path ?? 'Unavailable' },
    ],
    policyRows: [
      { label: 'Credential bindings', value: countOrList(version.allowed_credential_binding_ids) },
      { label: 'File workspaces', value: countOrList(version.allowed_file_workspace_ids) },
      { label: 'Extensions', value: countOrList(version.allowed_extension_ids) },
    ],
    schemaRows: [
      { label: 'Input schema', value: summarizeJson(version.input_schema) },
      { label: 'Output schema', value: summarizeJson(version.output_schema) },
      { label: 'Default session', value: summarizeJson(version.default_session) },
    ],
    packageRows: version.package
      ? [
          { label: 'Package', value: version.package.package_id },
          { label: 'Format', value: version.package.format_version },
          {
            label: 'Runtime',
            value: `Node ${version.package.runtime.node_major_version} · Playwright ${version.package.runtime.playwright_major_version}.${version.package.runtime.playwright_minor_version}`,
          },
          { label: 'Reviewer', value: version.package.publication.reviewer },
          { label: 'Reviewed', value: formatDateTime(version.package.publication.reviewed_at) },
          { label: 'Scenarios', value: scenarioSummary(version.package.publication.scenarios) },
        ]
      : [{ label: 'Manifest', value: 'No Phase 0 package manifest' }],
    compatibilityRows: [
      { label: 'State', value: version.compatibility.state },
      {
        label: 'Warnings',
        value: version.compatibility.warnings.length
          ? version.compatibility.warnings.join(' ')
          : 'No compatibility warnings',
      },
    ],
  };
}

function scenarioSummary(
  scenarios: readonly { readonly kind: string; readonly result: string }[],
): string {
  return scenarios.map(({ kind, result }) => `${kind}: ${result}`).join(', ');
}

function labelRows(labels: Readonly<Record<string, string>>): readonly MetadataRow[] {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return [{ label: 'Labels', value: 'No labels' }];
  }
  return entries.map(([label, value]) => ({ label, value }));
}

function sourceSummary(version: WorkflowDefinitionVersionResource | null): string {
  if (!version) {
    return 'Version metadata unavailable';
  }
  if (!version.source) {
    return 'No source snapshot';
  }
  const root = version.source.root_path ? `:${version.source.root_path}` : '';
  return `${version.source.kind} ${version.source.repository_url}${root}`;
}

function countOrList(values: readonly string[]): string {
  if (values.length === 0) {
    return 'None allowed';
  }
  if (values.length <= 3) {
    return values.join(', ');
  }
  return `${values.length} allowed`;
}

function metric(key: string, label: string, value: number): WorkflowOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `workflows-metric-${key}`,
  };
}
