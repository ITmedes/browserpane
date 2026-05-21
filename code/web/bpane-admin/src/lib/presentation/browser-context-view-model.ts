import type { BrowserContextResource, SessionResource } from '../api/control-types';
import { formatSessionFileTimestamp } from './session-file-format';

export type BrowserContextCatalogRowViewModel = {
  readonly id: string;
  readonly shortId: string;
  readonly name: string;
  readonly description: string;
  readonly labels: string;
  readonly persistence: string;
  readonly state: string;
  readonly createdAt: string;
  readonly updatedAt: string;
  readonly lastUsedAt: string;
  readonly deletedAt: string;
  readonly sessionCount: number;
  readonly activeSessionCount: number;
  readonly activeSessionId: string | null;
  readonly sessionSummary: string;
  readonly activeRuntimeSummary: string;
  readonly profileStorageSummary: string;
  readonly retentionSummary: string;
  readonly canDelete: boolean;
  readonly deleteHint: string;
};

export type BrowserContextCatalogViewModel = {
  readonly rows: readonly BrowserContextCatalogRowViewModel[];
  readonly selectedContext: BrowserContextCatalogRowViewModel | null;
  readonly totalCount: number;
  readonly readyCount: number;
  readonly deletedCount: number;
  readonly emptyMessage: string;
  readonly apiExample: string;
  readonly secretWarning: string;
};

export class BrowserContextViewModelBuilder {
  static catalog(input: {
    readonly contexts: readonly BrowserContextResource[];
    readonly sessions?: readonly SessionResource[];
    readonly selectedContextId?: string | null;
    readonly search?: string;
  }): BrowserContextCatalogViewModel {
    const normalized = (input.search ?? '').trim().toLowerCase();
    const sessionUsage = contextUsage(input.sessions ?? []);
    const rows = input.contexts
      .map((context) => toRow(context, contextUsageForContext(context, sessionUsage.get(context.id))))
      .filter((row) => rowMatches(row, normalized));
    const selectedContext = rows.find((row) => row.id === input.selectedContextId) ?? rows[0] ?? null;
    return {
      rows,
      selectedContext,
      totalCount: input.contexts.length,
      readyCount: input.contexts.filter((context) => context.state === 'ready').length,
      deletedCount: input.contexts.filter((context) => context.state === 'deleted').length,
      emptyMessage: normalized
        ? 'No browser contexts match the current filter.'
        : 'No reusable browser contexts are available yet.',
      apiExample: selectedContext ? apiExample(selectedContext.id) : 'Select a browser context to preview API calls.',
      secretWarning: 'Browser contexts preserve browser-side state only. Keep source credentials in credential bindings.',
    };
  }
}

type ContextUsage = {
  readonly sessionCount: number;
  readonly activeSessionCount: number;
  readonly activeSessionId: string | null;
  readonly profileStorageBytes: number | null;
};

function contextUsage(sessions: readonly SessionResource[]): ReadonlyMap<string, ContextUsage> {
  const usage = new Map<string, ContextUsage>();
  for (const session of sessions) {
    const contextId = session.browser_context?.mode === 'reusable'
      ? session.browser_context.context_id
      : null;
    if (!contextId) {
      continue;
    }
    const current = usage.get(contextId) ?? emptyUsage();
    const active = session.status.runtime_state === 'running' || session.status.presence_state === 'connected';
    const next = {
      ...current,
      sessionCount: current.sessionCount + 1,
      activeSessionCount: current.activeSessionCount + (active ? 1 : 0),
      activeSessionId: current.activeSessionId ?? (active ? session.id : null),
    };
    usage.set(contextId, next);
  }
  return usage;
}

function contextUsageForContext(context: BrowserContextResource, sessionUsage: ContextUsage | undefined): ContextUsage {
  const apiUsage = context.usage
    ? {
        sessionCount: context.usage.visible_session_count,
        activeSessionCount: context.usage.active_runtime_session_count,
        activeSessionId: context.usage.active_runtime_session_id ?? null,
        profileStorageBytes: context.usage.profile_storage_bytes ?? null,
      }
    : emptyUsage();
  if (!sessionUsage) {
    return apiUsage;
  }
  return {
    sessionCount: Math.max(apiUsage.sessionCount, sessionUsage.sessionCount),
    activeSessionCount: Math.max(apiUsage.activeSessionCount, sessionUsage.activeSessionCount),
    activeSessionId: apiUsage.activeSessionId ?? sessionUsage.activeSessionId,
    profileStorageBytes: apiUsage.profileStorageBytes,
  };
}

function emptyUsage(): ContextUsage {
  return {
    sessionCount: 0,
    activeSessionCount: 0,
    activeSessionId: null,
    profileStorageBytes: null,
  };
}

function toRow(context: BrowserContextResource, usage: ContextUsage): BrowserContextCatalogRowViewModel {
  const activeSessionCount = usage.activeSessionCount;
  const sessionCount = usage.sessionCount;
  const activeSessionId = usage.activeSessionId;
  const canDelete = context.state === 'ready' && sessionCount === 0 && activeSessionCount === 0;
  return {
    id: context.id,
    shortId: shortId(context.id),
    name: context.name,
    description: context.description ?? 'No description available.',
    labels: labelSummary(context.labels),
    persistence: context.persistence_mode,
    state: context.state,
    createdAt: formatOptionalTimestamp(context.created_at, 'not created'),
    updatedAt: formatOptionalTimestamp(context.updated_at, 'not updated'),
    lastUsedAt: formatOptionalTimestamp(context.last_used_at, 'never used'),
    deletedAt: formatOptionalTimestamp(context.deleted_at, 'not deleted'),
    sessionCount,
    activeSessionCount,
    activeSessionId,
    sessionSummary: `${sessionCount} visible session${sessionCount === 1 ? '' : 's'}`
      + (activeSessionCount > 0 ? `, ${activeSessionCount} active runtime` : ''),
    activeRuntimeSummary: activeRuntimeSummary(activeSessionCount, activeSessionId),
    profileStorageSummary: formatBytes(usage.profileStorageBytes),
    retentionSummary: retentionSummary(context),
    canDelete,
    deleteHint: deleteHint(context, sessionCount, activeSessionCount),
  };
}

function deleteHint(
  context: BrowserContextResource,
  sessionCount: number,
  activeSessionCount: number,
): string {
  if (context.state === 'deleted') {
    return 'Context is already deleted.';
  }
  if (activeSessionCount > 0) {
    return 'Disconnect or stop active sessions before deleting this context.';
  }
  if (sessionCount > 0) {
    return 'Delete is disabled while visible sessions still reference this context.';
  }
  return 'Deletes the catalog entry and the docker-backed Chromium profile data when present.';
}

function rowMatches(row: BrowserContextCatalogRowViewModel, normalized: string): boolean {
  if (!normalized) {
    return true;
  }
  return [
    row.id,
    row.shortId,
    row.name,
    row.description,
    row.labels,
    row.persistence,
    row.state,
    row.sessionSummary,
    row.activeRuntimeSummary,
    row.profileStorageSummary,
    row.retentionSummary,
  ].some((value) => value.toLowerCase().includes(normalized));
}

function labelSummary(labels: Readonly<Record<string, string>>): string {
  const entries = Object.entries(labels).sort(([left], [right]) => left.localeCompare(right));
  if (entries.length === 0) {
    return 'No labels';
  }
  return entries.map(([key, value]) => `${key}=${value}`).join(', ');
}

function formatOptionalTimestamp(value: string | null | undefined, fallback: string): string {
  return value ? formatSessionFileTimestamp(value) : fallback;
}

function retentionSummary(context: BrowserContextResource): string {
  const retentionSec = context.retention_sec ?? null;
  if (!retentionSec) {
    return 'manual retention';
  }
  const duration = formatDuration(retentionSec);
  const expiresAt = formatOptionalTimestamp(context.retention_expires_at, 'expiry unknown');
  return `${duration}, expires ${expiresAt}`;
}

function apiExample(contextId: string): string {
  return [
    `GET /api/v1/browser-contexts/${contextId}`,
    `DELETE /api/v1/browser-contexts/${contextId}`,
    [
      'POST /api/v1/sessions',
      JSON.stringify({
        browser_context: {
          mode: 'reusable',
          context_id: contextId,
        },
      }, null, 2),
    ].join('\n'),
  ].join('\n\n');
}

function activeRuntimeSummary(activeSessionCount: number, activeSessionId: string | null): string {
  if (activeSessionId) {
    return `session ${shortId(activeSessionId)}`;
  }
  if (activeSessionCount > 0) {
    return `${activeSessionCount} active runtime writer${activeSessionCount === 1 ? '' : 's'}`;
  }
  return 'none';
}

function formatBytes(value: number | null): string {
  if (value === null || !Number.isFinite(value)) {
    return 'unknown';
  }
  if (value < 1000) {
    return `${value} B`;
  }
  const units = ['kB', 'MB', 'GB', 'TB'];
  let scaled = value;
  let unit = 'B';
  for (const nextUnit of units) {
    scaled /= 1000;
    unit = nextUnit;
    if (scaled < 1000) {
      break;
    }
  }
  const precision = scaled >= 100 ? 0 : scaled >= 10 ? 1 : 2;
  return `${scaled.toFixed(precision)} ${unit}`;
}

function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return 'manual retention';
  }
  if (seconds % 86400 === 0) {
    const days = seconds / 86400;
    return `${days} day${days === 1 ? '' : 's'}`;
  }
  if (seconds % 3600 === 0) {
    const hours = seconds / 3600;
    return `${hours} hour${hours === 1 ? '' : 's'}`;
  }
  return `${seconds} seconds`;
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
