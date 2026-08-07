import { describe, expect, it } from 'vitest';

import { browserContextLifecycleEligibility } from './browser-context-lifecycle-view-model';
import type { BrowserContextResource } from './browser-context-types';

describe('browserContextLifecycleEligibility', () => {
  it('allows inactive reusable contexts even when inactive sessions still reference them', () => {
    const eligibility = browserContextLifecycleEligibility(context({
      visible_session_count: 3,
      active_runtime_session_count: 0,
    }));

    expect(eligibility).toMatchObject({
      canClone: true,
      canExport: true,
      cloneBlockedReason: null,
      exportBlockedReason: null,
    });
  });

  it('blocks deleted, ephemeral, and actively written contexts with stable reasons', () => {
    expect(browserContextLifecycleEligibility(context({}, { state: 'deleted' }))).toMatchObject({
      canClone: false,
      cloneBlockedReason: 'Deleted contexts cannot be cloned or exported.',
    });
    expect(browserContextLifecycleEligibility(context({}, { persistence_mode: 'ephemeral' }))).toMatchObject({
      canExport: false,
      exportBlockedReason: 'Only reusable contexts can be cloned or exported.',
    });
    expect(browserContextLifecycleEligibility(context({
      active_runtime_session_count: 1,
      active_runtime_session_id: 'session-1',
    }))).toMatchObject({
      canClone: false,
      canExport: false,
      activeSessionId: 'session-1',
      cloneBlockedReason: 'Stop the active browser session before cloning or exporting this context.',
    });
  });

  it('warns about over-limit storage without removing the export recovery path', () => {
    const eligibility = browserContextLifecycleEligibility(context({
      profile_storage_limit_exceeded: true,
    }));

    expect(eligibility.canClone).toBe(true);
    expect(eligibility.canExport).toBe(true);
    expect(eligibility.storageWarning).toContain('Export remains available for recovery');
  });
});

function context(
  usage: Partial<BrowserContextResource['usage']> = {},
  resource: Partial<BrowserContextResource> = {},
): BrowserContextResource {
  return {
    id: 'context-1',
    project_id: null,
    project: null,
    name: 'Support baseline',
    description: null,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: null,
    retention_expires_at: null,
    max_profile_storage_bytes: null,
    state: 'ready',
    usage: {
      visible_session_count: 0,
      active_runtime_session_count: 0,
      active_runtime_session_id: null,
      profile_storage_bytes: null,
      profile_storage_limit_exceeded: false,
      ...usage,
    },
    created_at: '2026-08-07T10:00:00Z',
    updated_at: '2026-08-07T10:00:00Z',
    last_used_at: null,
    deleted_at: null,
    ...resource,
  };
}
