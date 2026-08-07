import { describe, expect, it } from 'vitest';

import {
  browserContextEditDraftFromResource,
  createNewBrowserContextEditDraft,
  hasBrowserContextEditChanges,
  hasNewBrowserContextEditChanges,
  mergeProjectsWithSelected,
  validateBrowserContextEdit,
} from './browser-context-edit-view-model';
import type { BrowserContextResource } from './browser-context-types';
import { buildBrowserContextStatusSummaryModel } from './browser-context-edit-view-model';

describe('browser context edit view model', () => {
  it('builds an owner-scoped reusable create request', () => {
    const draft = {
      ...createNewBrowserContextEditDraft(),
      name: 'Support baseline',
      description: '  Shared support profile  ',
      labelsText: 'team=support\nprofile=baseline',
    };

    const validation = validateBrowserContextEdit(draft);

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({
      project_id: null,
      name: 'Support baseline',
      description: 'Shared support profile',
      labels: { team: 'support', profile: 'baseline' },
      persistence_mode: 'reusable',
      retention_sec: 604800,
      max_profile_storage_bytes: null,
    });
    expect(hasNewBrowserContextEditChanges(draft)).toBe(true);
  });

  it('builds a project-scoped context with storage limits', () => {
    const draft = {
      ...createNewBrowserContextEditDraft(),
      name: 'Customer profile',
      projectBinding: 'project' as const,
      projectId: 'project-1',
      storageLimitEnabled: true,
      maxProfileStorageBytes: '1073741824',
    };

    const validation = validateBrowserContextEdit(draft);

    expect(validation.valid).toBe(true);
    expect(validation.request).toMatchObject({
      project_id: 'project-1',
      max_profile_storage_bytes: 1073741824,
    });
  });

  it('validates missing names, project binding, labels, and positive integers', () => {
    const draft = {
      ...createNewBrowserContextEditDraft(),
      labelsText: 'missing-separator',
      projectBinding: 'project' as const,
      retentionSec: '0',
      storageLimitEnabled: true,
      maxProfileStorageBytes: '1.5',
    };

    const validation = validateBrowserContextEdit(draft);

    expect(validation.valid).toBe(false);
    expect(validation.fieldErrors.name).toContain('Name is required.');
    expect(validation.fieldErrors.projectId).toContain('Project-scoped contexts need a project.');
    expect(validation.fieldErrors.labels).toContain('Label line 1 must use key=value syntax.');
    expect(validation.fieldErrors.retentionSec).toContain('Retention seconds must be greater than zero.');
    expect(validation.fieldErrors.maxProfileStorageBytes).toContain('Profile storage limit must be a whole number.');
  });

  it('keeps a missing selected project visible in project options', () => {
    const projects = mergeProjectsWithSelected([], 'project-1');

    expect(projects).toEqual([{
      id: 'project-1',
      name: 'Missing project project-...',
      state: 'archived',
    }]);
  });

  it('creates a stable prefilled draft from an existing context', () => {
    const initialDraft = browserContextEditDraftFromResource(context(), 'Support baseline copy');

    expect(initialDraft).toEqual({
      name: 'Support baseline copy',
      description: '',
      labelsText: '',
      projectBinding: 'project',
      projectId: 'project-1',
      persistenceMode: 'reusable',
      retentionEnabled: true,
      retentionSec: '604800',
      storageLimitEnabled: true,
      maxProfileStorageBytes: '1024',
    });
    expect(hasBrowserContextEditChanges({ ...initialDraft }, initialDraft)).toBe(false);
    expect(hasBrowserContextEditChanges({ ...initialDraft, name: 'Changed' }, initialDraft)).toBe(true);
  });

  it('builds a status summary with usage and storage posture', () => {
    const model = buildBrowserContextStatusSummaryModel(context());

    expect(model.contextId).toBe('context-1');
    expect(model.items.map((item) => item.label)).toEqual([
      'Scope',
      'Retention',
      'Storage',
      'References',
      'Runtime',
    ]);
    expect(model.items.find((item) => item.label === 'Storage')?.tone).toBe('danger');
  });
});

function context(): BrowserContextResource {
  return {
    id: 'context-1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    name: 'Support baseline',
    description: null,
    labels: {},
    persistence_mode: 'reusable',
    retention_sec: 604800,
    retention_expires_at: null,
    max_profile_storage_bytes: 1024,
    state: 'ready',
    usage: {
      visible_session_count: 1,
      active_runtime_session_count: 1,
      active_runtime_session_id: 'session-1',
      profile_storage_bytes: 2048,
      profile_storage_limit_exceeded: true,
    },
    created_at: '2026-06-18T09:00:00.000Z',
    updated_at: '2026-06-18T10:00:00.000Z',
    last_used_at: null,
    deleted_at: null,
  };
}
