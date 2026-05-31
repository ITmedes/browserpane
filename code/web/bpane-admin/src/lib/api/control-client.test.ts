import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from './control-client';

const SESSION = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
  project: {
    id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    name: 'Support tenant',
    state: 'active',
  },
  admission: {
    state: 'allowed',
    reason_code: 'project_quota_available',
    message: 'Project admission allowed.',
    project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    active_sessions: 1,
    max_active_sessions: 2,
    checked_at: '2026-05-04T19:00:00Z',
  },
  template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
  browser_context: {
    mode: 'reusable',
    context_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
  },
  network_identity: {
    locale: 'de-DE',
    languages: ['de-DE', 'en-US'],
    timezone: 'Europe/Berlin',
    geolocation: { latitude: 52.52, longitude: 13.405, accuracy_meters: 100 },
    user_agent: null,
    browser_identity: 'desktop-chromium-stable',
    egress_profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
  },
  effective_egress: {
    profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
    profile_name: 'EU support egress',
    profile_state: 'ready',
    proxy_configured: true,
    proxy_auth_configured: false,
    bypass_rule_count: 2,
    custom_ca_configured: true,
    observation_mode: 'tls_intercept',
    tls_interception_enabled: true,
    sensitive_log_sink_configured: true,
  },
  owner_mode: 'shared',
  integration_context: { ticket: 'INC-1234' },
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
  stopped_at: null,
};

const BROWSER_CONTEXT = {
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
  name: 'Support profile',
  description: 'Reusable profile for support triage',
  labels: { team: 'support' },
  persistence_mode: 'reusable',
  retention_sec: 86400,
  retention_expires_at: '2026-05-05T18:30:00Z',
  max_profile_storage_bytes: 1048576,
  state: 'ready',
  usage: {
    visible_session_count: 1,
    active_runtime_session_count: 1,
    active_runtime_session_id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
    profile_storage_bytes: 1250000,
    profile_storage_limit_exceeded: true,
  },
  created_at: '2026-05-04T18:30:00Z',
  updated_at: '2026-05-04T18:30:00Z',
  last_used_at: null,
  deleted_at: null,
};

const PROJECT = {
  id: '019df811-91a5-7b00-9fe5-93403ea57f19',
  name: 'Support tenant',
  description: 'Support tenant project',
  labels: { tenant: 'support' },
  quotas: {
    max_active_sessions: 2,
    max_active_workflow_runs: 4,
    max_retained_storage_bytes: 1073741824,
  },
  state: 'active',
  usage: {
    project_id: '019df811-91a5-7b00-9fe5-93403ea57f19',
    active_sessions: 1,
    max_active_sessions: 2,
    active_workflow_runs: 1,
    max_active_workflow_runs: 4,
    retained_storage_bytes: 268435456,
    max_retained_storage_bytes: 1073741824,
    observed_at: '2026-05-04T18:50:00Z',
  },
  created_at: '2026-05-04T18:50:00Z',
  updated_at: '2026-05-04T18:50:00Z',
};

const IDENTITY_PRINCIPAL = {
  subject: 'demo',
  issuer: 'http://localhost:8091/realms/browserpane',
  display_name: 'demo',
  client_id: 'bpane-web',
  principal_type: 'user',
};

const SERVICE_PRINCIPAL = {
  id: '019df8ec-a5e2-7b00-a14b-173e70c39f7d',
  name: 'MCP bridge',
  description: 'Bridge automation identity',
  client_id: 'bpane-mcp-bridge',
  issuer: 'http://localhost:8091/realms/browserpane',
  labels: { system: 'mcp' },
  scopes: ['session:delegate'],
  allowed_project_ids: ['019df811-91a5-7b00-9fe5-93403ea57f19'],
  state: 'active',
  last_seen_at: null,
  last_delegated_at: '2026-05-29T10:00:00Z',
  created_at: '2026-05-29T09:00:00Z',
  updated_at: '2026-05-29T10:00:00Z',
};

const IDENTITY_MAPPING = {
  id: '019df8ec-a5e2-7b00-a14b-173e70c39f7e',
  name: 'Demo project access',
  description: null,
  kind: 'user',
  issuer: 'http://localhost:8091/realms/browserpane',
  external_id: 'demo',
  claim_name: null,
  service_principal_id: null,
  project_id: PROJECT.id,
  labels: { team: 'support' },
  scopes: ['session:create'],
  state: 'active',
  last_seen_at: null,
  created_at: '2026-05-29T09:00:00Z',
  updated_at: '2026-05-29T10:00:00Z',
};

const ACCESS_REVIEW = {
  principal: IDENTITY_PRINCIPAL,
  generated_at: '2026-05-29T10:00:00Z',
  projects: [PROJECT],
  resource_counts: {
    projects: 1,
    service_principals: 1,
    identity_mappings: 1,
    sessions: 2,
    active_sessions: 1,
    session_templates: 1,
    browser_contexts: 1,
    egress_profiles: 1,
    credential_bindings: 0,
    file_workspaces: 1,
    workflow_definitions: 1,
    workflow_runs: 2,
    active_workflow_runs: 1,
    automation_tasks: 2,
    active_automation_tasks: 1,
    extension_definitions: 0,
    delegated_principals: 1,
  },
  identity_mappings: [
    {
      ...IDENTITY_MAPPING,
      effective_for_principal: true,
    },
  ],
  unmapped_principal_signals: [],
  service_principals: [
    {
      ...SERVICE_PRINCIPAL,
      delegated_session_count: 1,
      active_delegated_session_count: 1,
      delegated_session_ids: [SESSION.id],
    },
  ],
  delegated_principals: [
    {
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/browserpane',
      display_name: 'BrowserPane MCP bridge',
      registered: true,
      registered_service_principal_id: SERVICE_PRINCIPAL.id,
      state: 'active',
      session_count: 1,
      active_session_count: 1,
      session_ids: [SESSION.id],
    },
  ],
};

const TEMPLATE = {
  id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
  name: 'Support triage',
  description: 'Default support case browser session',
  labels: { team: 'support' },
  defaults: {
    owner_mode: 'collaborative',
    idle_timeout_sec: 1800,
    labels: { team: 'support' },
    integration_context: { source: 'template' },
    network_identity: {
      locale: 'de-DE',
      languages: ['de-DE'],
      timezone: 'Europe/Berlin',
      egress_profile_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
    },
  },
  version: 1,
  created_at: '2026-05-04T18:00:00Z',
  updated_at: '2026-05-04T18:00:00Z',
};

const EGRESS_PROFILE = {
  id: '019df7be-6222-7b00-8c86-9e1f3f8d4a73',
  name: 'EU support egress',
  description: 'Approved support outbound path',
  labels: { region: 'eu' },
  proxy: { url: 'https://proxy.example:8443' },
  bypass_rules: ['localhost', '*.internal.example'],
  custom_ca: {
    certificate_ref: 'vault://pki/browserpane/eu-support',
    display_name: 'EU support CA',
  },
  traffic_observation: {
    mode: 'tls_intercept',
    sensitive_log_sink_ref: 'siem://browserpane/eu-support',
    sensitive_log_sink_display_name: 'EU support SIEM',
  },
  state: 'ready',
  effective: {
    proxy_configured: true,
    proxy_auth_configured: false,
    bypass_rule_count: 2,
    custom_ca_configured: true,
    observation_mode: 'tls_intercept',
    tls_interception_enabled: true,
    sensitive_log_sink_configured: true,
  },
  created_at: '2026-05-04T18:45:00Z',
  updated_at: '2026-05-04T18:45:00Z',
};

describe('ControlClient', () => {
  it('lists owner-visible sessions with bearer auth', async () => {
    const fetchImpl = jsonFetch({ sessions: [SESSION] });
    const client = new ControlClient({
      baseUrl: 'https://browserpane.example/app/',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.listSessions();

    expect(response.sessions).toHaveLength(1);
    expect(response.sessions[0]?.id).toBe(SESSION.id);
    expect(response.sessions[0]?.project?.name).toBe('Support tenant');
    expect(response.sessions[0]?.admission?.reason_code).toBe('project_quota_available');
    expect(response.sessions[0]?.browser_context).toEqual(SESSION.browser_context);
    expect(response.sessions[0]?.network_identity?.locale).toBe('de-DE');
    expect(response.sessions[0]?.effective_egress?.profile_name).toBe('EU support egress');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('https://browserpane.example/api/v1/sessions'),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          accept: 'application/json',
          authorization: 'Bearer owner-token',
        }),
      }),
    );
  });

  it('loads identity and access-review resources with bearer auth', async () => {
    const responses = [IDENTITY_PRINCIPAL, ACCESS_REVIEW];
    const fetchImpl = vi.fn<FetchLike>(async () => new Response(JSON.stringify(responses.shift()), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }));
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const principal = await client.getCurrentIdentity();
    const review = await client.getIdentityAccessReview();

    expect(principal).toMatchObject({
      subject: 'demo',
      principal_type: 'user',
    });
    expect(review).toMatchObject({
      principal: { subject: 'demo', principal_type: 'user' },
      resource_counts: {
        projects: 1,
        service_principals: 1,
        identity_mappings: 1,
        sessions: 2,
        delegated_principals: 1,
      },
      identity_mappings: [
        {
          id: IDENTITY_MAPPING.id,
          effective_for_principal: true,
        },
      ],
      service_principals: [
        {
          id: SERVICE_PRINCIPAL.id,
          delegated_session_ids: [SESSION.id],
        },
      ],
      delegated_principals: [
        {
          client_id: 'bpane-mcp-bridge',
          registered: true,
          session_ids: [SESSION.id],
        },
      ],
    });
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/identity/me'),
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL('http://localhost:8932/api/v1/identity/access-review'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('manages service principal registry resources with bearer auth', async () => {
    const responses = [
      SERVICE_PRINCIPAL,
      { service_principals: [SERVICE_PRINCIPAL] },
      SERVICE_PRINCIPAL,
      { ...SERVICE_PRINCIPAL, name: 'MCP bridge v2' },
    ];
    const fetchImpl = vi.fn<FetchLike>(async () => new Response(JSON.stringify(responses.shift()), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }));
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const created = await client.createServicePrincipal({
      name: 'MCP bridge',
      description: 'Bridge automation identity',
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/browserpane',
      labels: { system: 'mcp' },
      scopes: ['session:delegate'],
      allowed_project_ids: ['019df811-91a5-7b00-9fe5-93403ea57f19'],
    });
    const listed = await client.listServicePrincipals();
    await client.getServicePrincipal('service/principal with space');
    await client.updateServicePrincipal(SERVICE_PRINCIPAL.id, {
      name: 'MCP bridge v2',
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/browserpane',
      state: 'active',
    });

    expect(created.id).toBe(SERVICE_PRINCIPAL.id);
    expect(listed.service_principals).toHaveLength(1);
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/service-principals'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'MCP bridge',
          description: 'Bridge automation identity',
          client_id: 'bpane-mcp-bridge',
          issuer: 'http://localhost:8091/realms/browserpane',
          labels: { system: 'mcp' },
          scopes: ['session:delegate'],
          allowed_project_ids: ['019df811-91a5-7b00-9fe5-93403ea57f19'],
          state: 'active',
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL('http://localhost:8932/api/v1/service-principals'),
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      new URL('http://localhost:8932/api/v1/service-principals/service%2Fprincipal%20with%20space'),
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      4,
      new URL(`http://localhost:8932/api/v1/service-principals/${SERVICE_PRINCIPAL.id}`),
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({
          name: 'MCP bridge v2',
          client_id: 'bpane-mcp-bridge',
          issuer: 'http://localhost:8091/realms/browserpane',
          state: 'active',
          labels: {},
          scopes: [],
          allowed_project_ids: [],
        }),
      }),
    );
  });

  it('manages project catalog resources with bearer auth', async () => {
    const responses = [PROJECT, PROJECT, { ...PROJECT, name: 'Support tenant archived', state: 'archived' }, PROJECT.usage];
    const fetchImpl = vi.fn<FetchLike>(async () => new Response(JSON.stringify(responses.shift()), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }));
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const created = await client.createProject({
      name: 'Support tenant',
      description: 'Support tenant project',
      labels: { tenant: 'support' },
      quotas: {
        max_active_sessions: 2,
        max_active_workflow_runs: 4,
        max_retained_storage_bytes: 1073741824,
      },
    });
    await client.getProject('project/with space');
    await client.updateProject('project/with space', {
      name: 'Support tenant archived',
      state: 'archived',
    });
    await client.getProjectUsage('project/with space');

    expect(created).toMatchObject({
      id: PROJECT.id,
      name: 'Support tenant',
      usage: {
        active_sessions: 1,
        max_active_sessions: 2,
      },
    });
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/projects'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'Support tenant',
          description: 'Support tenant project',
          labels: { tenant: 'support' },
          quotas: {
            max_active_sessions: 2,
            max_active_workflow_runs: 4,
            max_retained_storage_bytes: 1073741824,
          },
          state: 'active',
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL('http://localhost:8932/api/v1/projects/project%2Fwith%20space'),
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      new URL('http://localhost:8932/api/v1/projects/project%2Fwith%20space'),
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({
          name: 'Support tenant archived',
          state: 'archived',
          labels: {},
          quotas: {},
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      4,
      new URL('http://localhost:8932/api/v1/projects/project%2Fwith%20space/usage'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('lists project catalog resources with bearer auth', async () => {
    const fetchImpl = jsonFetch({ projects: [PROJECT] });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.listProjects();

    expect(response.projects[0]).toMatchObject({
      id: PROJECT.id,
      name: 'Support tenant',
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/projects'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('manages browser context catalog resources with bearer auth', async () => {
    const fetchImpl = jsonFetch(BROWSER_CONTEXT);
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const created = await client.createBrowserContext({
      name: 'Support profile',
      labels: { team: 'support' },
      retention_sec: 86400,
      max_profile_storage_bytes: 1048576,
    });
    await client.cloneBrowserContext(BROWSER_CONTEXT.id, {
      name: 'Support profile sandbox',
      labels: { copy: 'sandbox' },
    });
    await client.getBrowserContext('context/with space');
    const exported = await client.exportBrowserContext(BROWSER_CONTEXT.id);
    await client.importBrowserContext({
      name: 'Support profile imported',
      archive: new Blob(['PKbrowser-context-export'], { type: 'application/zip' }),
      labels: { imported: 'true' },
      retention_sec: 43200,
    });
    await client.deleteBrowserContext(BROWSER_CONTEXT.id);
    expect(await exported.text()).toBe(JSON.stringify(BROWSER_CONTEXT));

    expect(created).toMatchObject({
      id: BROWSER_CONTEXT.id,
      name: 'Support profile',
      persistence_mode: 'reusable',
      retention_sec: 86400,
      retention_expires_at: '2026-05-05T18:30:00Z',
      max_profile_storage_bytes: 1048576,
      state: 'ready',
      usage: {
        visible_session_count: 1,
        active_runtime_session_count: 1,
        active_runtime_session_id: SESSION.id,
        profile_storage_bytes: 1250000,
        profile_storage_limit_exceeded: true,
      },
    });
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/browser-contexts'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'Support profile',
          labels: { team: 'support' },
          retention_sec: 86400,
          max_profile_storage_bytes: 1048576,
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL(`http://localhost:8932/api/v1/browser-contexts/${BROWSER_CONTEXT.id}/clone`),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'Support profile sandbox',
          labels: { copy: 'sandbox' },
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      new URL('http://localhost:8932/api/v1/browser-contexts/context%2Fwith%20space'),
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      4,
      new URL(`http://localhost:8932/api/v1/browser-contexts/${BROWSER_CONTEXT.id}/export`),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({ accept: 'application/zip' }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      5,
      new URL('http://localhost:8932/api/v1/browser-contexts/import'),
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({
          accept: 'application/json',
          'content-type': 'application/zip',
          'x-bpane-browser-context-name': 'Support profile imported',
          'x-bpane-browser-context-labels': JSON.stringify({ imported: 'true' }),
          'x-bpane-browser-context-retention-sec': '43200',
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      6,
      new URL(`http://localhost:8932/api/v1/browser-contexts/${BROWSER_CONTEXT.id}`),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  it('lists browser contexts with bearer auth', async () => {
    const fetchImpl = jsonFetch({ contexts: [BROWSER_CONTEXT] });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.listBrowserContexts();

    expect(response.contexts[0]).toMatchObject({
      id: BROWSER_CONTEXT.id,
      name: 'Support profile',
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/browser-contexts'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('lists session templates with bearer auth', async () => {
    const fetchImpl = jsonFetch({ templates: [TEMPLATE] });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.listSessionTemplates();

    expect(response.templates[0]).toMatchObject({
      id: TEMPLATE.id,
      name: 'Support triage',
      defaults: expect.objectContaining({
        idle_timeout_sec: 1800,
        network_identity: expect.objectContaining({
          locale: 'de-DE',
        }),
      }),
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/session-templates'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('manages egress profiles with bearer auth', async () => {
    const fetchImpl = jsonFetch(EGRESS_PROFILE);
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const created = await client.createEgressProfile({
      name: 'EU support egress',
      labels: { region: 'eu' },
      proxy: { url: 'https://proxy.example:8443' },
      bypass_rules: ['localhost', '*.internal.example'],
      custom_ca: {
        certificate_ref: 'vault://pki/browserpane/eu-support',
        display_name: 'EU support CA',
      },
      traffic_observation: {
        mode: 'tls_intercept',
        sensitive_log_sink_ref: 'siem://browserpane/eu-support',
        sensitive_log_sink_display_name: 'EU support SIEM',
      },
    });
    await client.getEgressProfile('profile/with space');
    await client.updateEgressProfile('profile/with space', {
      name: 'EU support egress disabled',
      state: 'disabled',
    });

    expect(created).toMatchObject({
      id: EGRESS_PROFILE.id,
      name: 'EU support egress',
      effective: {
        proxy_configured: true,
        proxy_auth_configured: false,
        bypass_rule_count: 2,
        custom_ca_configured: true,
        observation_mode: 'tls_intercept',
        tls_interception_enabled: true,
        sensitive_log_sink_configured: true,
      },
    });
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/egress-profiles'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'EU support egress',
          labels: { region: 'eu' },
          proxy: { url: 'https://proxy.example:8443' },
          bypass_rules: ['localhost', '*.internal.example'],
          custom_ca: {
            certificate_ref: 'vault://pki/browserpane/eu-support',
            display_name: 'EU support CA',
          },
          traffic_observation: {
            mode: 'tls_intercept',
            sensitive_log_sink_ref: 'siem://browserpane/eu-support',
            sensitive_log_sink_display_name: 'EU support SIEM',
          },
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL('http://localhost:8932/api/v1/egress-profiles/profile%2Fwith%20space'),
      expect.objectContaining({ method: 'GET' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      new URL('http://localhost:8932/api/v1/egress-profiles/profile%2Fwith%20space'),
      expect.objectContaining({ method: 'PUT' }),
    );
    expect(JSON.parse(fetchImpl.mock.calls[2]?.[1]?.body as string)).toEqual({
      name: 'EU support egress disabled',
      labels: {},
      bypass_rules: [],
      state: 'disabled',
    });
  });

  it('runs session egress diagnostics probes with bearer auth', async () => {
    const fetchImpl = jsonFetch({
      profile_id: EGRESS_PROFILE.id,
      profile_name: EGRESS_PROFILE.name,
      profile_state: 'ready',
      health: 'ready',
      observation_mode: 'tls_intercept',
      proof_level: 'active_probe',
      runtime_binding: 'docker_runtime_pool',
      runtime_assignment: 'ready',
      proxy_configured: true,
      proxy_auth_configured: false,
      bypass_rule_count: 2,
      custom_ca_configured: true,
      tls_interception_enabled: true,
      sensitive_log_sink_configured: true,
      proof: {
        profile_resolved: true,
        profile_ready: true,
        profile_reachability_collected: true,
        profile_reachability_healthy: true,
        profile_reachability_observed_at: '2026-05-22T09:29:00Z',
        profile_reachability_failure: null,
        proxy_launch_config_expected: true,
        bypass_rules_expected: 2,
        custom_ca_launch_config_expected: true,
        tls_interception_expected: true,
        sensitive_log_sink_declared: true,
        runtime_launch_observed: true,
        active_probe_collected: true,
        observed_public_ip: '203.0.113.10',
        observed_tls_issuer: 'BrowserPane Local Egress Test CA',
        last_failure_reason: null,
      },
      warnings: [],
      observed_at: '2026-05-22T09:30:00Z',
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const diagnostics = await client.runSessionEgressDiagnosticsProbe('session/with space', {
      public_ip_url: 'https://probe.example/ip',
      tls_probe_url: 'https://probe.example/tls',
      timeout_ms: 1000,
    });

    expect(diagnostics.proof_level).toBe('active_probe');
    expect(diagnostics.proof.observed_public_ip).toBe('203.0.113.10');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions/session%2Fwith%20space/egress-diagnostics'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          public_ip_url: 'https://probe.example/ip',
          tls_probe_url: 'https://probe.example/tls',
          timeout_ms: 1000,
        }),
      }),
    );
  });

  it('runs egress profile reachability probes with bearer auth', async () => {
    const fetchImpl = jsonFetch({
      profile_id: EGRESS_PROFILE.id,
      profile_name: EGRESS_PROFILE.name,
      profile_state: 'ready',
      health: 'ready',
      observation_mode: 'tls_intercept',
      proof_level: 'active_probe',
      runtime_binding: null,
      runtime_assignment: null,
      proxy_configured: true,
      proxy_auth_configured: false,
      bypass_rule_count: 2,
      custom_ca_configured: true,
      tls_interception_enabled: true,
      sensitive_log_sink_configured: true,
      proof: {
        profile_resolved: true,
        profile_ready: true,
        profile_reachability_collected: true,
        profile_reachability_healthy: true,
        profile_reachability_observed_at: '2026-05-22T09:29:00Z',
        profile_reachability_failure: null,
        proxy_launch_config_expected: true,
        bypass_rules_expected: 2,
        custom_ca_launch_config_expected: true,
        tls_interception_expected: true,
        sensitive_log_sink_declared: true,
        runtime_launch_observed: false,
        active_probe_collected: false,
        observed_public_ip: null,
        observed_tls_issuer: null,
        last_failure_reason: null,
      },
      warnings: [],
      observed_at: '2026-05-22T09:30:00Z',
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const diagnostics = await client.runEgressProfileReachabilityProbe('profile/with space', {
      timeout_ms: 1000,
    });

    expect(diagnostics.proof.profile_reachability_healthy).toBe(true);
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/egress-profiles/profile%2Fwith%20space/diagnostics/probe'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ timeout_ms: 1000 }),
      }),
    );
  });

  it('passes catalog filters to the session list endpoint', async () => {
    const fetchImpl = jsonFetch({ sessions: [SESSION] });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    await client.listSessions({
      templateId: TEMPLATE.id,
      states: ['active'],
      runtimeStates: ['running'],
      labels: { team: 'support' },
      integrationContext: { ticket: 'INC-1234' },
      limit: 25,
      offset: 50,
    });

    const url = fetchImpl.mock.calls[0]?.[0] as URL;
    expect(url.pathname).toBe('/api/v1/sessions');
    expect(url.searchParams.get('template_id')).toBe(TEMPLATE.id);
    expect(url.searchParams.get('state')).toBe('active');
    expect(url.searchParams.get('runtime_state')).toBe('running');
    expect(url.searchParams.get('label.team')).toBe('support');
    expect(url.searchParams.get('integration.ticket')).toBe('INC-1234');
    expect(url.searchParams.get('limit')).toBe('25');
    expect(url.searchParams.get('offset')).toBe('50');
  });

  it('creates sessions through the frozen v1 endpoint', async () => {
    const fetchImpl = jsonFetch(SESSION);
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    await client.createSession({
      project_id: PROJECT.id,
      template_id: TEMPLATE.id,
      browser_context: {
        mode: 'reusable',
        context_id: BROWSER_CONTEXT.id,
      },
      network_identity: {
        locale: 'de-DE',
        languages: ['de-DE', 'en-US'],
        timezone: 'Europe/Berlin',
        egress_profile_id: EGRESS_PROFILE.id,
      },
      idle_timeout_sec: 300,
      labels: { source: 'admin-smoke' },
    });

    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          project_id: PROJECT.id,
          template_id: TEMPLATE.id,
          browser_context: {
            mode: 'reusable',
            context_id: BROWSER_CONTEXT.id,
          },
          network_identity: {
            locale: 'de-DE',
            languages: ['de-DE', 'en-US'],
            timezone: 'Europe/Berlin',
            egress_profile_id: EGRESS_PROFILE.id,
          },
          idle_timeout_sec: 300,
          labels: { source: 'admin-smoke' },
        }),
        headers: expect.objectContaining({
          'content-type': 'application/json',
        }),
      }),
    );
  });

  it('encodes session ids for lifecycle operations', async () => {
    const fetchImpl = jsonFetch(SESSION);
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    await client.stopSession('session/with/slash');
    await client.releaseSessionRuntime('session/with/slash');

    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions/session%2Fwith%2Fslash/stop'),
      expect.objectContaining({ method: 'POST' }),
    );
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions/session%2Fwith%2Fslash/release'),
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('issues session-scoped connect tickets', async () => {
    const fetchImpl = jsonFetch({
      session_id: SESSION.id,
      token_type: 'session_connect_ticket',
      token: 'connect-ticket',
      expires_at: '2026-05-04T19:05:00Z',
      connect: SESSION.connect,
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const response = await client.issueSessionAccessToken(SESSION.id);

    expect(response.token).toBe('connect-ticket');
    expect(response.connect.auth_type).toBe('session_connect_ticket');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION.id}/access-tokens`),
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('sets and clears a session automation delegate', async () => {
    const fetchImpl = jsonFetch({
      ...SESSION,
      automation_delegate: {
        client_id: 'bpane-mcp-bridge',
        issuer: 'http://localhost:8091/realms/bpane',
        display_name: 'BrowserPane MCP bridge',
      },
    });
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      fetchImpl,
    });

    const delegated = await client.setAutomationDelegate(SESSION.id, {
      client_id: 'bpane-mcp-bridge',
      issuer: 'http://localhost:8091/realms/bpane',
      display_name: 'BrowserPane MCP bridge',
    });
    await client.clearAutomationDelegate(SESSION.id);

    expect(delegated.automation_delegate?.client_id).toBe('bpane-mcp-bridge');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION.id}/automation-owner`),
      expect.objectContaining({ method: 'POST' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL(`http://localhost:8932/api/v1/sessions/${SESSION.id}/automation-owner`),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  it('throws a typed API error for non-success responses', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => new Response('denied', { status: 403 }));
    const onAuthenticationFailure = vi.fn();
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      onAuthenticationFailure,
      fetchImpl,
    });

    await expect(client.listSessions()).rejects.toMatchObject({ status: 403, body: 'denied' });
    expect(onAuthenticationFailure).not.toHaveBeenCalled();
  });

  it('notifies the app about expired owner bearer auth', async () => {
    const fetchImpl = vi.fn<FetchLike>(async () => new Response('expired', { status: 401 }));
    const onAuthenticationFailure = vi.fn();
    const client = new ControlClient({
      baseUrl: 'http://localhost:8932',
      accessTokenProvider: () => 'owner-token',
      onAuthenticationFailure,
      fetchImpl,
    });

    await expect(client.listSessions()).rejects.toMatchObject({ status: 401 });
    expect(onAuthenticationFailure).toHaveBeenCalledWith(expect.objectContaining({ status: 401, body: 'expired' }));
  });
});

function jsonFetch(payload: unknown): ReturnType<typeof vi.fn<FetchLike>> {
  return vi.fn<FetchLike>(async () => new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  }));
}
