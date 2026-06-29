import { describe, expect, it } from 'vitest';

import {
  browserContextMatchesSearch,
  buildBrowserContextOverviewModel,
} from './browser-context-overview-view-model';
import type { BrowserContextResource } from './browser-context-types';

describe('browser context overview view model', () => {
  it('builds metrics and rows from reusable, active, and deleted contexts', () => {
    const model = buildBrowserContextOverviewModel([
      context({ id: 'ready', name: 'Ready baseline' }),
      context({ id: 'active', name: 'Active profile', visibleSessions: 2, activeRuntimeSessions: 1 }),
      context({ id: 'deleted', name: 'Deleted profile', state: 'deleted', storageOverLimit: true }),
    ]);

    expect(model.metrics).toEqual([
      { label: 'Contexts', value: '3', testId: 'browser-contexts-metric-total' },
      { label: 'Ready', value: '2', testId: 'browser-contexts-metric-ready' },
      { label: 'Active use', value: '1', testId: 'browser-contexts-metric-active' },
      { label: 'Needs attention', value: '1', testId: 'browser-contexts-metric-attention' },
    ]);
    expect(model.rows.map((row) => ({
      id: row.id,
      persistenceLabel: row.persistenceLabel,
      scope: row.scope,
      usageTone: row.usageTone,
      storageTone: row.storageTone,
    }))).toEqual([
      { id: 'ready', persistenceLabel: 'Reusable', scope: 'Owner scoped', usageTone: 'neutral', storageTone: 'success' },
      { id: 'active', persistenceLabel: 'Reusable', scope: 'Owner scoped', usageTone: 'warning', storageTone: 'success' },
      { id: 'deleted', persistenceLabel: 'Reusable', scope: 'Owner scoped', usageTone: 'neutral', storageTone: 'danger' },
    ]);
  });

  it('matches search against scope, retention, storage, and usage fields', () => {
    const row = buildBrowserContextOverviewModel([
      context({ id: 'context-1', name: 'EU Support', projectName: 'Customer Support', visibleSessions: 1 }),
    ]).rows[0];

    expect(row).toBeDefined();
    if (!row) {
      return;
    }

    expect(browserContextMatchesSearch(row, 'customer')).toBe(true);
    expect(browserContextMatchesSearch(row, 'reusable')).toBe(true);
    expect(browserContextMatchesSearch(row, 'visible sessions')).toBe(true);
    expect(browserContextMatchesSearch(row, 'missing')).toBe(false);
  });
});

function context(options: {
  readonly id: string;
  readonly name: string;
  readonly state?: BrowserContextResource['state'];
  readonly projectName?: string;
  readonly visibleSessions?: number;
  readonly activeRuntimeSessions?: number;
  readonly storageOverLimit?: boolean;
}): BrowserContextResource {
  return {
    id: options.id,
    project_id: options.projectName ? 'project-1' : null,
    project: options.projectName ? { id: 'project-1', name: options.projectName, state: 'active' } : null,
    name: options.name,
    description: `${options.name} profile`,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: 1073741824,
    state: options.state ?? 'ready',
    usage: {
      visible_session_count: options.visibleSessions ?? 0,
      active_runtime_session_count: options.activeRuntimeSessions ?? 0,
      active_runtime_session_id: null,
      profile_storage_bytes: 1048576,
      profile_storage_limit_exceeded: options.storageOverLimit ?? false,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
