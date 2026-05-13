import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
  WorkflowRunResource,
} from '../api/workflow-types';
import { workflowDefinitionKind } from '../application/workflow-definition-visibility';

export type WorkflowCatalogItem = {
  readonly definition: WorkflowDefinitionResource;
  readonly latestVersion: WorkflowDefinitionVersionResource | null;
  readonly versionError: string | null;
};

export type WorkflowCatalogViewModel = {
  readonly rows: readonly WorkflowCatalogRow[];
  readonly totalCount: number;
  readonly hiddenCount: number;
  readonly emptyMessage: string;
};

export type WorkflowCatalogRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly kind: string;
  readonly latestVersion: string;
  readonly executor: string;
  readonly source: string;
  readonly sourceCommit: string;
  readonly labels: string;
  readonly updatedAt: string;
};

export type WorkflowDefinitionDetailViewModel = {
  readonly definitionId: string;
  readonly name: string;
  readonly description: string;
  readonly kind: string;
  readonly latestVersion: string;
  readonly labels: readonly MetadataRow[];
  readonly versionRows: readonly WorkflowVersionRow[];
  readonly selectedVersion: WorkflowVersionDetail | null;
  readonly recentRuns: readonly WorkflowRunRow[];
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
};

export type WorkflowRunRow = {
  readonly id: string;
  readonly state: string;
  readonly version: string;
  readonly sessionId: string;
  readonly updatedAt: string;
};

export type MetadataRow = {
  readonly label: string;
  readonly value: string;
};

export class WorkflowTemplateCatalogViewModelBuilder {
  static catalog(input: {
    readonly items: readonly WorkflowCatalogItem[];
    readonly hiddenCount: number;
    readonly search: string;
  }): WorkflowCatalogViewModel {
    const normalized = input.search.trim().toLowerCase();
    const rows = input.items
      .map(toCatalogRow)
      .filter((row) => matchesCatalogSearch(row, normalized));
    return {
      rows,
      totalCount: input.items.length,
      hiddenCount: input.hiddenCount,
      emptyMessage: normalized
        ? 'No workflow templates match the current filter.'
        : 'No workflow templates are available yet.',
    };
  }

  static detail(input: {
    readonly definition: WorkflowDefinitionResource;
    readonly versions: readonly WorkflowDefinitionVersionResource[];
    readonly selectedVersion: string;
    readonly runs: readonly WorkflowRunResource[];
  }): WorkflowDefinitionDetailViewModel {
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
      labels: toLabelRows(input.definition.labels),
      versionRows: input.versions.map((version) => ({
        id: version.id,
        version: version.version,
        executor: version.executor,
        createdAt: formatDate(version.created_at),
        latest: version.version === input.definition.latest_version,
      })),
      selectedVersion: selected ? toVersionDetail(selected) : null,
      recentRuns: input.runs
        .filter((run) => run.workflow_definition_id === input.definition.id)
        .slice()
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
        .slice(0, 8)
        .map((run) => ({
          id: run.id,
          state: run.state,
          version: run.workflow_version,
          sessionId: run.session_id,
          updatedAt: formatDate(run.updated_at),
        })),
    };
  }
}

function toCatalogRow(item: WorkflowCatalogItem): WorkflowCatalogRow {
  const definition = item.definition;
  return {
    id: definition.id,
    name: definition.name,
    description: definition.description ?? 'No description available.',
    kind: workflowDefinitionKind(definition),
    latestVersion: definition.latest_version ?? 'No version',
    executor: item.latestVersion?.executor ?? item.versionError ?? 'Version metadata unavailable',
    source: sourceSummary(item.latestVersion),
    sourceCommit: item.latestVersion?.source?.resolved_commit ?? 'Not pinned',
    labels: labelSummary(definition.labels),
    updatedAt: formatDate(definition.updated_at),
  };
}

function toVersionDetail(version: WorkflowDefinitionVersionResource): WorkflowVersionDetail {
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
      {
        label: 'Credential bindings',
        value: countOrList(version.allowed_credential_binding_ids),
      },
      { label: 'File workspaces', value: countOrList(version.allowed_file_workspace_ids) },
      { label: 'Extensions', value: countOrList(version.allowed_extension_ids) },
    ],
    schemaRows: [
      { label: 'Input schema', value: summarizeJson(version.input_schema) },
      { label: 'Output schema', value: summarizeJson(version.output_schema) },
      { label: 'Default session', value: summarizeJson(version.default_session) },
    ],
  };
}

function matchesCatalogSearch(row: WorkflowCatalogRow, normalized: string): boolean {
  if (!normalized) {
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
  ].some((value) => value.toLowerCase().includes(normalized));
}

function toLabelRows(labels: Readonly<Record<string, string>>): readonly MetadataRow[] {
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

function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
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

function summarizeJson(value: unknown): string {
  if (value === undefined || value === null) {
    return 'Unavailable';
  }
  const serialized = JSON.stringify(value);
  if (!serialized) {
    return 'Unavailable';
  }
  return serialized.length > 160 ? `${serialized.slice(0, 160)}...` : serialized;
}

function formatDate(value: string | null | undefined): string {
  return value ? new Date(value).toLocaleString() : 'Unavailable';
}
