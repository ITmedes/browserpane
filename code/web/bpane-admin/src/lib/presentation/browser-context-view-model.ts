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
  readonly sessionSummary: string;
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
    const usage = contextUsage(input.sessions ?? []);
    const rows = input.contexts
      .map((context) => toRow(context, usage.get(context.id) ?? emptyUsage()))
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
};

function contextUsage(sessions: readonly SessionResource[]): ReadonlyMap<string, ContextUsage> {
  const usage = new Map<string, { sessionCount: number; activeSessionCount: number }>();
  for (const session of sessions) {
    const contextId = session.browser_context?.mode === 'reusable'
      ? session.browser_context.context_id
      : null;
    if (!contextId) {
      continue;
    }
    const current = usage.get(contextId) ?? { sessionCount: 0, activeSessionCount: 0 };
    current.sessionCount += 1;
    if (session.status.runtime_state === 'running' || session.status.presence_state === 'connected') {
      current.activeSessionCount += 1;
    }
    usage.set(contextId, current);
  }
  return usage;
}

function emptyUsage(): ContextUsage {
  return { sessionCount: 0, activeSessionCount: 0 };
}

function toRow(context: BrowserContextResource, usage: ContextUsage): BrowserContextCatalogRowViewModel {
  const activeSessionCount = usage.activeSessionCount;
  const sessionCount = usage.sessionCount;
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
    sessionSummary: `${sessionCount} visible session${sessionCount === 1 ? '' : 's'}`
      + (activeSessionCount > 0 ? `, ${activeSessionCount} active` : ''),
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

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
