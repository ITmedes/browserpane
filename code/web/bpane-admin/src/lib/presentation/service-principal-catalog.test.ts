import { describe, expect, it } from 'vitest';
import type {
  IdentityPrincipalResource,
  IdentityServicePrincipalReviewResource,
  ProjectResource,
} from '../api/control-types';
import {
  buildServicePrincipalCommand,
  commandFromServicePrincipal,
  emptyServicePrincipalForm,
  formFromServicePrincipal,
  servicePrincipalRows,
} from './service-principal-catalog';

const PROJECT: ProjectResource = {
  id: '019df811-91a5-7b00-9fe5-93403ea57f19',
  name: 'Acme support',
  description: null,
  labels: {},
  quotas: {},
  policy: {
    allowed_session_template_ids: [],
    allowed_egress_profile_ids: [],
    allowed_extension_ids: [],
    allowed_browser_context_ids: [],
    allow_browser_uploads: true,
    allow_browser_downloads: true,
    allow_session_file_bindings: true,
    allow_manual_recordings: true,
    usage_budget_enforcement: 'warning_only',
  },
  state: 'active',
  usage: {
    project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    active_sessions: 0,
    queued_sessions: 0,
    session_creations: 0,
    active_workflow_runs: 0,
    runtime_usage_ms: 0,
    egress_rx_bytes: 0,
    egress_tx_bytes: 0,
    egress_total_bytes: 0,
    retained_storage_bytes: 0,
    alerts: [],
    observed_at: '2026-05-31T10:00:00Z',
  },
  created_at: '2026-05-31T10:00:00Z',
  updated_at: '2026-05-31T10:00:00Z',
};

const PRINCIPAL: IdentityPrincipalResource = {
  subject: 'demo',
  issuer: 'http://localhost:8091/realms/browserpane-dev',
  display_name: 'demo',
  client_id: 'bpane-web',
  principal_type: 'user',
};

const SERVICE_PRINCIPAL: IdentityServicePrincipalReviewResource = {
  id: '019df8ec-a5e2-7b00-a14b-173e70c39f7d',
  name: 'MCP bridge',
  description: 'Bridge automation identity',
  client_id: 'bpane-mcp-bridge',
  issuer: PRINCIPAL.issuer,
  labels: { system: 'mcp' },
  scopes: ['session:delegate'],
  allowed_project_ids: [PROJECT.id],
  state: 'active',
  last_seen_at: null,
  last_delegated_at: '2026-05-31T10:05:00Z',
  created_at: '2026-05-31T10:00:00Z',
  updated_at: '2026-05-31T10:00:00Z',
  delegated_session_count: 2,
  active_delegated_session_count: 1,
  delegated_session_ids: [],
};

describe('service principal catalog helpers', () => {
  it('builds searchable rows with project names', () => {
    const rows = servicePrincipalRows([SERVICE_PRINCIPAL], 'acme', [PROJECT]);

    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      name: 'MCP bridge',
      clientId: 'bpane-mcp-bridge',
      projects: 'Acme support (019df811...7f19)',
      delegatedSummary: '1/2 active',
    });
  });

  it('creates defaults from the signed-in principal issuer', () => {
    expect(emptyServicePrincipalForm(PRINCIPAL)).toMatchObject({
      name: 'Service principal',
      issuer: PRINCIPAL.issuer,
      state: 'active',
    });
  });

  it('round-trips registry entries into edit and disable commands', () => {
    expect(formFromServicePrincipal(SERVICE_PRINCIPAL)).toMatchObject({
      name: 'MCP bridge',
      labels: 'system=mcp',
      allowedProjectIds: [PROJECT.id],
    });
    expect(commandFromServicePrincipal(SERVICE_PRINCIPAL, 'disabled')).toMatchObject({
      name: 'MCP bridge',
      client_id: 'bpane-mcp-bridge',
      state: 'disabled',
    });
  });

  it('validates required fields and parses lists', () => {
    const result = buildServicePrincipalCommand({
      ...emptyServicePrincipalForm(PRINCIPAL),
      name: 'MCP bridge',
      clientId: 'bpane-mcp-bridge',
      labels: 'system=mcp',
      scopes: 'session:create\nsession:delegate',
      allowedProjectIds: [PROJECT.id],
    });

    expect(result).toEqual({
      ok: true,
      command: {
        name: 'MCP bridge',
        client_id: 'bpane-mcp-bridge',
        issuer: PRINCIPAL.issuer,
        labels: { system: 'mcp' },
        scopes: ['session:create', 'session:delegate'],
        allowed_project_ids: [PROJECT.id],
        state: 'active',
      },
    });
  });
});
