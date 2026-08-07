import { formatDateTime, type ProjectTone } from '$lib/projects/project-formatters';
import type {
  CredentialBindingProjectResource,
  CredentialBindingResource,
} from './credential-binding-types';
import { injectionModeLabel } from './credential-binding-form-model';

export type CredentialBindingOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly bindings: readonly CredentialBindingResource[] };

export type CredentialBindingDetailLoadState =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly bindingId: string }
  | { readonly status: 'error'; readonly bindingId: string; readonly message: string }
  | { readonly status: 'ready'; readonly binding: CredentialBindingResource };

export type CredentialBindingProjectOptionsLoadState =
  | { readonly status: 'idle' | 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | { readonly status: 'ready'; readonly projects: readonly CredentialBindingProjectResource[] };

export type CredentialBindingOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly scope: string;
  readonly scopeTone: ProjectTone;
  readonly provider: string;
  readonly injectionMode: string;
  readonly origins: string;
  readonly namespace: string;
  readonly labels: string;
  readonly updatedAt: string;
};

export function buildCredentialBindingOverviewModel(
  bindings: readonly CredentialBindingResource[],
) {
  return {
    metrics: [
      metric('total', 'Bindings', bindings.length),
      metric('owner', 'Owner scoped', bindings.filter((binding) => !binding.project_id).length),
      metric('project', 'Project scoped', bindings.filter((binding) => binding.project_id).length),
      metric(
        'totp',
        'TOTP',
        bindings.filter((binding) => binding.injection_mode === 'totp_fill').length,
      ),
    ],
    rows: bindings.map(credentialBindingOverviewRow),
  };
}

export function credentialBindingOverviewRow(
  binding: CredentialBindingResource,
): CredentialBindingOverviewRow {
  return {
    id: binding.id,
    name: binding.name,
    scope: binding.project?.name ?? binding.project_id ?? 'Owner scoped',
    scopeTone: binding.project_id ? 'warning' : 'neutral',
    provider: binding.provider === 'vault_kv_v2' ? 'Vault KV v2' : binding.provider,
    injectionMode: injectionModeLabel(binding.injection_mode),
    origins:
      binding.allowed_origins.length > 0
        ? binding.allowed_origins.join(', ')
        : 'No allowed origins',
    namespace: binding.namespace ?? 'Default namespace',
    labels: labelSummary(binding.labels),
    updatedAt: formatDateTime(binding.updated_at),
  };
}

export function credentialBindingMatchesSearch(
  row: CredentialBindingOverviewRow,
  query: string,
): boolean {
  if (!query) return true;
  return [
    row.id,
    row.name,
    row.scope,
    row.provider,
    row.injectionMode,
    row.origins,
    row.namespace,
    row.labels,
    row.updatedAt,
  ].some((value) => value.toLowerCase().includes(query));
}

export function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  return entries.length === 0
    ? 'No labels'
    : entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function metric(key: string, label: string, value: number) {
  return { label, value: String(value), testId: `credential-bindings-metric-${key}` };
}
