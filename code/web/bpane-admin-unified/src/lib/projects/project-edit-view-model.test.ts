import { describe, expect, it } from 'vitest';

import {
  createProjectEditDraft,
  hasProjectEditChanges,
  validateProjectEdit,
} from './project-edit-view-model';
import type { ProjectResource } from './project-types';

describe('project edit view model', () => {
  it('creates a draft and maps edited quotas and policy into the update request', () => {
    const source = project();
    const draft = {
      ...createProjectEditDraft(source),
      description: 'Updated support project',
      labelsText: 'env=prod\nteam=support',
      allowBrowserUploads: false,
      usageBudgetEnforcement: 'block_session_creation' as const,
      restrictSessionTemplates: true,
      allowedSessionTemplateIds: ['template-support'],
      restrictBrowserContexts: true,
      allowedBrowserContextIds: ['22222222-2222-4222-8222-222222222222'],
      maxActiveSessions: { enabled: true, value: '6' },
      maxSessionCreationsPerWindow: { enabled: true, value: '10' },
      sessionCreationWindowSec: { enabled: true, value: '3600' },
    };

    const validation = validateProjectEdit(source, draft);

    expect(validation.valid).toBe(true);
    expect(validation.request).toMatchObject({
      name: 'Support',
      description: 'Updated support project',
      labels: { env: 'prod', team: 'support' },
      quotas: expect.objectContaining({
        max_active_sessions: 6,
        max_session_creations_per_window: 10,
        session_creation_window_sec: 3600,
      }),
      policy: expect.objectContaining({
        allowed_session_template_ids: ['template-support'],
        allowed_browser_context_ids: ['22222222-2222-4222-8222-222222222222'],
        allow_browser_uploads: false,
        usage_budget_enforcement: 'block_session_creation',
      }),
      state: 'active',
    });
    expect(hasProjectEditChanges(source, draft)).toBe(true);
  });

  it('rejects empty names and invalid label lines', () => {
    const source = project();
    const validation = validateProjectEdit(project(), {
      ...createProjectEditDraft(source),
      name: ' ',
      labelsText: 'missing-separator\nteam=',
    });

    expect(validation.request).toBeNull();
    expect(validation.errors).toEqual([
      'Project name is required.',
      'Label line 1 must use key=value.',
      'Label line 2 has an empty value.',
    ]);
    expect(validation.fieldErrors).toMatchObject({
      name: ['Project name is required.'],
      labels: ['Label line 1 must use key=value.', 'Label line 2 has an empty value.'],
    });
  });

  it('rejects invalid quotas and empty restricted allow-lists', () => {
    const source = project();
    const validation = validateProjectEdit(source, {
      ...createProjectEditDraft(source),
      restrictSessionTemplates: true,
      allowedSessionTemplateIds: [],
      maxActiveSessions: { enabled: true, value: '0' },
      maxSessionCreationsPerWindow: { enabled: true, value: '3' },
      sessionCreationWindowSec: { enabled: false, value: '' },
    });

    expect(validation.request).toBeNull();
    expect(validation.errors).toEqual([
      'Active sessions quota needs a positive integer.',
      'Rolling session creation quota needs both session creations/window and session creation window enabled.',
      'Session Templates restriction needs at least one selected resource.',
    ]);
    expect(validation.fieldErrors).toMatchObject({
      maxActiveSessions: ['Active sessions quota needs a positive integer.'],
      maxSessionCreationsPerWindow: [
        'Rolling session creation quota needs both session creations/window and session creation window enabled.',
      ],
      sessionCreationWindowSec: [
        'Rolling session creation quota needs both session creations/window and session creation window enabled.',
      ],
      allowedSessionTemplateIds: ['Session Templates restriction needs at least one selected resource.'],
    });
  });
});

function project(): ProjectResource {
  return {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'Support',
    description: 'Support browser work',
    labels: { team: 'support' },
    quotas: { max_active_sessions: 4 },
    policy: {
      allowed_session_template_ids: [],
      allowed_egress_profile_ids: [],
      allowed_extension_ids: [],
      allowed_browser_context_ids: [],
      allowed_file_workspace_ids: [],
      allow_browser_uploads: true,
      allow_browser_downloads: true,
      allow_session_file_bindings: true,
      allow_manual_recordings: true,
      usage_budget_enforcement: 'warning_only',
    },
    state: 'active',
    usage: {
      project_id: '11111111-1111-4111-8111-111111111111',
      active_sessions: 1,
      queued_sessions: 0,
      session_creations: 1,
      max_session_creations: null,
      max_active_sessions: 4,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 30_000,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-06-11T10:00:00.000Z',
    },
    created_at: '2026-06-11T09:00:00.000Z',
    updated_at: '2026-06-11T10:00:00.000Z',
  };
}
