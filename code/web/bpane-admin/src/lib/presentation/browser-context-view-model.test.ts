import { describe, expect, it } from 'vitest';
import type { BrowserContextResource, SessionResource } from '../api/control-types';
import { BrowserContextViewModelBuilder } from './browser-context-view-model';

describe('BrowserContextViewModelBuilder', () => {
  it('summarizes context lifecycle state and guarded deletion', () => {
    const viewModel = BrowserContextViewModelBuilder.catalog({
      contexts: [CONTEXT],
      sessions: [SESSION],
      selectedContextId: CONTEXT.id,
    });

    expect(viewModel.readyCount).toBe(1);
    expect(viewModel.selectedContext?.name).toBe('Support profile');
    expect(viewModel.selectedContext?.sessionSummary).toBe('1 visible session, 1 active runtime');
    expect(viewModel.selectedContext?.profileStorageSummary).toBe('unknown');
    expect(viewModel.selectedContext?.profileStorageLimitSummary).toBe('no storage limit');
    expect(viewModel.selectedContext?.retentionSummary).toBe('manual retention');
    expect(viewModel.selectedContext?.canDelete).toBe(false);
    expect(viewModel.selectedContext?.deleteHint).toContain('active sessions');
    expect(viewModel.selectedContext?.canClone).toBe(false);
    expect(viewModel.selectedContext?.cloneHint).toContain('active sessions');
    expect(viewModel.apiExample).toContain(`/api/v1/browser-contexts/${CONTEXT.id}`);
    expect(viewModel.apiExample).toContain(`/api/v1/browser-contexts/${CONTEXT.id}/clone`);
    expect(viewModel.apiExample).toContain('"mode": "reusable"');
    expect(viewModel.secretWarning).toContain('credential bindings');
  });

  it('allows deletion only for unused ready contexts', () => {
    const viewModel = BrowserContextViewModelBuilder.catalog({
      contexts: [CONTEXT],
      sessions: [],
      selectedContextId: CONTEXT.id,
    });

    expect(viewModel.selectedContext?.canDelete).toBe(true);
    expect(viewModel.selectedContext?.deleteHint).toContain('Chromium profile data');
    expect(viewModel.selectedContext?.canClone).toBe(true);
    expect(viewModel.selectedContext?.cloneHint).toContain('copied metadata');
  });

  it('uses API usage when the session list is unavailable', () => {
    const viewModel = BrowserContextViewModelBuilder.catalog({
      contexts: [{
        ...CONTEXT,
        usage: {
          visible_session_count: 2,
          active_runtime_session_count: 1,
          active_runtime_session_id: SESSION.id,
          profile_storage_bytes: 1250000,
          profile_storage_limit_exceeded: true,
        },
        max_profile_storage_bytes: 1000000,
        retention_sec: 172800,
        retention_expires_at: '2026-05-06T18:45:00Z',
      }],
      selectedContextId: CONTEXT.id,
    });

    expect(viewModel.selectedContext?.sessionSummary).toBe('2 visible sessions, 1 active runtime');
    expect(viewModel.selectedContext?.activeRuntimeSummary).toContain('019df4d2');
    expect(viewModel.selectedContext?.profileStorageSummary).toBe('1.25 MB');
    expect(viewModel.selectedContext?.profileStorageLimitSummary).toContain('exceeded');
    expect(viewModel.selectedContext?.retentionSummary).toContain('2 days');
    expect(viewModel.selectedContext?.canDelete).toBe(false);
  });

  it('filters context rows by name, label, and usage text', () => {
    const viewModel = BrowserContextViewModelBuilder.catalog({
      contexts: [CONTEXT, DELETED_CONTEXT],
      sessions: [SESSION],
      search: 'team=support',
    });

    expect(viewModel.rows.map((row) => row.id)).toEqual([CONTEXT.id]);
    expect(viewModel.totalCount).toBe(2);
    expect(viewModel.deletedCount).toBe(1);
  });
});

const CONTEXT: BrowserContextResource = {
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
  name: 'Support profile',
  description: 'Reusable support auth state',
  labels: { team: 'support' },
  persistence_mode: 'reusable',
  retention_sec: null,
  retention_expires_at: null,
  state: 'ready',
  created_at: '2026-05-04T18:30:00Z',
  updated_at: '2026-05-04T18:40:00Z',
  last_used_at: '2026-05-04T18:45:00Z',
  deleted_at: null,
};

const DELETED_CONTEXT: BrowserContextResource = {
  ...CONTEXT,
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
  name: 'Deleted context',
  labels: {},
  state: 'deleted',
  deleted_at: '2026-05-04T18:50:00Z',
};

const SESSION: SessionResource = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  template_id: null,
  browser_context: {
    mode: 'reusable',
    context_id: CONTEXT.id,
  },
  owner_mode: 'collaborative',
  labels: {},
  integration_context: null,
  connect: {
    gateway_url: 'https://localhost:4433',
    transport_path: '/session',
    auth_type: 'session_connect_ticket',
    ticket_path: '/api/v1/sessions/019df4d2-f4f7-7b00-9e0c-79683b1c82f6/access-tokens',
    compatibility_mode: 'session_runtime_pool',
  },
  runtime: {
    binding: 'docker_runtime_pool',
    compatibility_mode: 'session_runtime_pool',
    cdp_endpoint: 'http://runtime:9223',
  },
  status: {
    runtime_state: 'running',
    runtime_resume_mode: 'exact_live',
    presence_state: 'connected',
    connection_counts: {
      interactive_clients: 1,
      owner_clients: 1,
      viewer_clients: 0,
      recorder_clients: 0,
      automation_clients: 0,
      total_clients: 1,
    },
    stop_eligibility: {
      allowed: false,
      blockers: [{ kind: 'owner_clients', count: 1 }],
    },
  },
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};
