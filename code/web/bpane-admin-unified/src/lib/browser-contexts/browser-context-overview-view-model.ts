import {
  formatBytes,
  formatDateTime,
  type ProjectTone,
} from '$lib/projects/project-formatters';
import type {
  BrowserContextPersistenceMode,
  BrowserContextResource,
  BrowserContextState,
} from './browser-context-types';

export type BrowserContextOverviewLoadState =
  | { readonly status: 'loading' }
  | { readonly status: 'error'; readonly message: string }
  | {
      readonly status: 'ready';
      readonly contexts: readonly BrowserContextResource[];
    };

export type BrowserContextOverviewMetric = {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
};

export type BrowserContextOverviewRow = {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly state: BrowserContextState;
  readonly stateTone: ProjectTone;
  readonly persistence: BrowserContextPersistenceMode;
  readonly persistenceLabel: string;
  readonly persistenceTone: ProjectTone;
  readonly scope: string;
  readonly retentionSummary: string;
  readonly storageSummary: string;
  readonly storageTone: ProjectTone;
  readonly usageSummary: string;
  readonly usageTone: ProjectTone;
  readonly lastUsedAt: string;
  readonly updatedAt: string;
  readonly badges: readonly string[];
};

export type BrowserContextOverviewModel = {
  readonly metrics: readonly BrowserContextOverviewMetric[];
  readonly rows: readonly BrowserContextOverviewRow[];
};

export function buildBrowserContextOverviewModel(
  contexts: readonly BrowserContextResource[],
): BrowserContextOverviewModel {
  return {
    metrics: [
      metric('total', 'Contexts', contexts.length),
      metric('ready', 'Ready', contexts.filter((context) => context.state === 'ready').length),
      metric('active', 'Active use', contexts.filter((context) =>
        context.usage.visible_session_count > 0 || context.usage.active_runtime_session_count > 0).length),
      metric('attention', 'Needs attention', contexts.filter((context) =>
        context.state === 'deleted' || context.usage.profile_storage_limit_exceeded).length),
    ],
    rows: contexts.map(browserContextRow),
  };
}

export function browserContextRow(context: BrowserContextResource): BrowserContextOverviewRow {
  const storageLimitExceeded = context.usage.profile_storage_limit_exceeded;
  const activeUse = context.usage.visible_session_count > 0 || context.usage.active_runtime_session_count > 0;
  return {
    id: context.id,
    name: context.name,
    description: context.description?.trim() || context.id,
    state: context.state,
    stateTone: context.state === 'ready' ? 'success' : 'neutral',
    persistence: context.persistence_mode,
    persistenceLabel: context.persistence_mode === 'reusable' ? 'Reusable' : 'Ephemeral',
    persistenceTone: context.persistence_mode === 'reusable' ? 'success' : 'neutral',
    scope: context.project?.name ?? context.project_id ?? 'Owner scoped',
    retentionSummary: retentionSummary(context),
    storageSummary: storageSummary(context),
    storageTone: storageLimitExceeded ? 'danger' : context.max_profile_storage_bytes ? 'success' : 'neutral',
    usageSummary: usageSummary(context),
    usageTone: activeUse ? 'warning' : 'neutral',
    lastUsedAt: context.last_used_at ? formatDateTime(context.last_used_at) : 'Never',
    updatedAt: formatDateTime(context.updated_at),
    badges: browserContextBadges(context),
  };
}

export function browserContextMatchesSearch(
  row: BrowserContextOverviewRow,
  normalizedQuery: string,
): boolean {
  if (!normalizedQuery) {
    return true;
  }
  return [
    row.id,
    row.name,
    row.description,
    row.state,
    row.persistenceLabel,
    row.scope,
    row.retentionSummary,
    row.storageSummary,
    row.usageSummary,
    row.lastUsedAt,
    ...row.badges,
  ].some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function retentionSummary(context: BrowserContextResource): string {
  if (!context.retention_sec) {
    return 'No retention expiry';
  }
  const duration = formatSeconds(context.retention_sec);
  return context.retention_expires_at
    ? `${duration} · expires ${formatDateTime(context.retention_expires_at)}`
    : duration;
}

export function storageSummary(context: BrowserContextResource): string {
  const current = formatBytes(context.usage.profile_storage_bytes) ?? 'unknown';
  const limit = formatBytes(context.max_profile_storage_bytes);
  if (!limit) {
    return `${current} / unbounded`;
  }
  return `${current} / ${limit}${context.usage.profile_storage_limit_exceeded ? ' · over limit' : ''}`;
}

function usageSummary(context: BrowserContextResource): string {
  return [
    `${context.usage.visible_session_count} visible sessions`,
    `${context.usage.active_runtime_session_count} active runtime`,
  ].join(' · ');
}

function browserContextBadges(context: BrowserContextResource): readonly string[] {
  return [
    context.persistence_mode,
    context.project_id ? 'project scoped' : 'owner scoped',
    context.state,
    context.usage.visible_session_count > 0 ? 'referenced' : null,
    context.usage.active_runtime_session_count > 0 ? 'active runtime' : null,
    context.usage.profile_storage_limit_exceeded ? 'storage over limit' : null,
    context.retention_sec ? 'retention' : null,
    context.max_profile_storage_bytes ? 'storage limit' : null,
  ].filter((value): value is string => Boolean(value));
}

function formatSeconds(value: number): string {
  const days = Math.floor(value / 86400);
  const hours = Math.floor((value % 86400) / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  if (days > 0) {
    return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
  }
  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }
  if (minutes > 0) {
    return `${minutes}m`;
  }
  return `${value}s`;
}

function metric(key: string, label: string, value: number): BrowserContextOverviewMetric {
  return {
    label,
    value: String(value),
    testId: `browser-contexts-metric-${key}`,
  };
}
