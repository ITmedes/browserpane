import { describe, expect, it } from 'vitest';

import type { ProjectPolicyOption, ProjectResource } from '$lib/projects/project-types';
import {
  createNewSessionCreateDraft,
  hasSessionCreateDraftChanges,
  validateSessionCreateDraft,
} from './session-create-view-model';

describe('session create view model', () => {
  it('keeps the empty draft as an empty create-session request', () => {
    const draft = createNewSessionCreateDraft();
    const validation = validateSessionCreateDraft(draft);

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({});
    expect(validation.preview).toBe('{}');
    expect(hasSessionCreateDraftChanges(draft)).toBe(false);
  });

  it('builds a create-session request only from explicit operator input', () => {
    const draft = {
      ...createNewSessionCreateDraft(),
      projectId: 'project-1',
      templateId: 'template-1',
      ownerMode: 'exclusive_browser_owner' as const,
      browserContextMode: 'reusable' as const,
      browserContextId: 'context-1',
      egressProfileId: 'egress-1',
      idleTimeoutSec: '900',
      viewportWidth: '1440',
      viewportHeight: '900',
      locale: 'en-US',
      languagesText: 'en-US, de-DE',
      timezone: 'Europe/Berlin',
      labelsText: 'team=support\npurpose=smoke',
    };

    const validation = validateSessionCreateDraft(draft, {
      projects: [project({ id: 'project-1' })],
      sessionTemplates: [option({ id: 'template-1' })],
      browserContexts: [option({ id: 'context-1' })],
      egressProfiles: [option({ id: 'egress-1' })],
    });

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({
      project_id: 'project-1',
      template_id: 'template-1',
      owner_mode: 'exclusive_browser_owner',
      browser_context: { mode: 'reusable', context_id: 'context-1' },
      idle_timeout_sec: 900,
      viewport: { width: 1440, height: 900 },
      labels: { team: 'support', purpose: 'smoke' },
      network_identity: {
        locale: 'en-US',
        languages: ['en-US', 'de-DE'],
        timezone: 'Europe/Berlin',
        egress_profile_id: 'egress-1',
      },
    });
    expect(JSON.stringify(validation.request)).not.toContain('bpane_admin_surface');
  });

  it('adds capability overrides only after a default is changed', () => {
    const validation = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      capabilityClipboard: false,
      capabilityMicrophone: false,
      capabilityCamera: false,
    });

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({
      capabilities: {
        browser_input: true,
        clipboard: false,
        audio: true,
        microphone: false,
        camera: false,
        file_transfer: true,
        resize: true,
      },
    });
  });

  it('adds always-on recording only when the operator enables recording', () => {
    const validation = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      recordingEnabled: true,
      recordingRetentionSec: '86400',
    });

    expect(validation.valid).toBe(true);
    expect(validation.request).toEqual({
      recording: {
        mode: 'always',
        format: 'webm',
        retention_sec: 86400,
      },
    });
  });

  it('rejects invalid recording retention values', () => {
    const validation = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      recordingEnabled: true,
      recordingRetentionSec: '0',
    });

    expect(validation.valid).toBe(false);
    expect(validation.fieldErrors.recordingRetentionSec).toContain(
      'Recording retention must be a positive whole number.',
    );
  });

  it('rejects invalid metadata references and malformed labels', () => {
    const validation = validateSessionCreateDraft({
      ...createNewSessionCreateDraft(),
      projectId: 'project-archived',
      browserContextMode: 'reusable',
      labelsText: 'missing-separator',
      viewportWidth: '1024',
    }, {
      projects: [project({ id: 'project-archived', state: 'archived' })],
      sessionTemplates: [],
      browserContexts: [],
      egressProfiles: [],
    });

    expect(validation.valid).toBe(false);
    expect(validation.fieldErrors.projectId).toContain('Archived projects cannot be used for new sessions.');
    expect(validation.fieldErrors.browserContextId).toContain('Reusable browser context requires a selected context.');
    expect(validation.fieldErrors.labelsText?.[0]).toContain('key=value');
    expect(validation.fieldErrors.viewport).toContain('Viewport width and height must be provided together.');
  });
});

function option(overrides: Partial<ProjectPolicyOption> = {}): ProjectPolicyOption {
  return {
    id: 'option-1',
    name: 'Option',
    description: null,
    state: 'ready',
    scope: 'owner scoped',
    ...overrides,
  };
}

function project(overrides: Partial<ProjectResource> = {}): ProjectResource {
  const id = overrides.id ?? 'project-1';
  return {
    id,
    name: 'Project',
    description: null,
    labels: {},
    quotas: {},
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
      project_id: id,
      active_sessions: 0,
      queued_sessions: 0,
      session_creations: 0,
      max_session_creations: null,
      max_active_sessions: null,
      active_workflow_runs: 0,
      max_active_workflow_runs: null,
      runtime_usage_ms: 0,
      max_runtime_usage_ms: null,
      egress_rx_bytes: 0,
      egress_tx_bytes: 0,
      egress_total_bytes: 0,
      max_egress_total_bytes: null,
      retained_storage_bytes: 0,
      max_retained_storage_bytes: null,
      alerts: [],
      observed_at: '2026-06-29T12:00:00Z',
    },
    created_at: '2026-06-29T12:00:00Z',
    updated_at: '2026-06-29T12:00:00Z',
    ...overrides,
  };
}
