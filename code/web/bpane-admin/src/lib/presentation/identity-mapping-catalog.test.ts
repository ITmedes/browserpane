import { describe, expect, it } from 'vitest';
import type {
  IdentityMappingResource,
  IdentityPrincipalResource,
  IdentityServicePrincipalReviewResource,
  ProjectResource,
} from '../api/control-types';
import {
  buildIdentityMappingCommand,
  commandFromIdentityMapping,
  emptyIdentityMappingForm,
  formFromIdentityMapping,
  formWithServicePrincipal,
  identityMappingRows,
} from './identity-mapping-catalog';

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
    allowed_file_workspace_ids: [],
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
  description: null,
  client_id: 'bpane-mcp-bridge',
  issuer: 'http://localhost:8091/realms/browserpane-dev',
  labels: {},
  scopes: ['session:delegate'],
  allowed_project_ids: [PROJECT.id],
  state: 'active',
  last_seen_at: null,
  last_delegated_at: null,
  created_at: '2026-05-31T10:00:00Z',
  updated_at: '2026-05-31T10:00:00Z',
  delegated_session_count: 0,
  active_delegated_session_count: 0,
  delegated_session_ids: [],
};

const MAPPING: IdentityMappingResource = {
  id: '019df8ec-a5e2-7b00-a14b-173e70c39f7e',
  name: 'MCP bridge mapping',
  description: 'Automation project access',
  kind: 'service_principal',
  issuer: SERVICE_PRINCIPAL.issuer,
  external_id: SERVICE_PRINCIPAL.client_id,
  claim_name: null,
  service_principal_id: SERVICE_PRINCIPAL.id,
  project_id: PROJECT.id,
  labels: { system: 'mcp' },
  scopes: ['session:create'],
  state: 'active',
  last_seen_at: null,
  created_at: '2026-05-31T10:00:00Z',
  updated_at: '2026-05-31T10:05:00Z',
};

describe('identity mapping catalog helpers', () => {
  it('builds searchable rows from access-review mappings', () => {
    const rows = identityMappingRows([{ ...MAPPING, effective_for_principal: true }], 'mcp', [PROJECT]);

    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      id: MAPPING.id,
      kind: 'Service principal',
      externalIdentity: 'bpane-mcp-bridge',
      projectId: 'Acme support (019df811...7f19)',
      effective: 'effective',
    });
  });

  it('creates a default user mapping form from the current principal', () => {
    expect(emptyIdentityMappingForm(PRINCIPAL, [PROJECT])).toMatchObject({
      kind: 'user',
      issuer: PRINCIPAL.issuer,
      externalId: PRINCIPAL.subject,
      projectId: PROJECT.id,
      state: 'active',
    });
  });

  it('fills service-principal mapping fields from the selected registry entry', () => {
    expect(formWithServicePrincipal(emptyIdentityMappingForm(PRINCIPAL, [PROJECT]), SERVICE_PRINCIPAL)).toMatchObject({
      kind: 'service_principal',
      servicePrincipalId: SERVICE_PRINCIPAL.id,
      issuer: SERVICE_PRINCIPAL.issuer,
      externalId: SERVICE_PRINCIPAL.client_id,
    });
  });

  it('creates valid commands for claim mappings', () => {
    const result = buildIdentityMappingCommand({
      name: 'Support group claim',
      description: 'Maps group claim to project',
      kind: 'claim',
      issuer: PRINCIPAL.issuer,
      externalId: 'customer-acme-support',
      claimName: 'groups',
      servicePrincipalId: '',
      projectId: PROJECT.id,
      labels: 'team=support',
      scopes: 'session:create\nsession:delegate',
      state: 'active',
    });

    expect(result).toEqual({
      ok: true,
      command: {
        name: 'Support group claim',
        description: 'Maps group claim to project',
        kind: 'claim',
        issuer: PRINCIPAL.issuer,
        external_id: 'customer-acme-support',
        claim_name: 'groups',
        project_id: PROJECT.id,
        labels: { team: 'support' },
        scopes: ['session:create', 'session:delegate'],
        state: 'active',
      },
    });
  });

  it('validates kind-specific fields before saving', () => {
    const result = buildIdentityMappingCommand({
      ...emptyIdentityMappingForm(PRINCIPAL, [PROJECT]),
      kind: 'service_principal',
    });

    expect(result).toEqual({
      ok: false,
      error: 'Service-principal mappings require a registered service principal.',
    });
  });

  it('round-trips existing mappings into edit and disable commands', () => {
    expect(formFromIdentityMapping(MAPPING)).toMatchObject({
      name: MAPPING.name,
      servicePrincipalId: SERVICE_PRINCIPAL.id,
      labels: 'system=mcp',
    });
    expect(commandFromIdentityMapping(MAPPING, 'disabled')).toMatchObject({
      name: MAPPING.name,
      service_principal_id: SERVICE_PRINCIPAL.id,
      state: 'disabled',
    });
  });
});
