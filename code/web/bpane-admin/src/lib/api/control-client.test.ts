import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from './control-client';

const SESSION = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  state: 'active',
  template_id: '019df5c8-3d03-7800-9e5d-79d69d9a21c0',
  browser_context: {
    mode: 'reusable',
    context_id: '019df7be-6222-7b00-8c86-9e1f3f8d4a72',
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
  },
  version: 1,
  created_at: '2026-05-04T18:00:00Z',
  updated_at: '2026-05-04T18:00:00Z',
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
    expect(response.sessions[0]?.browser_context).toEqual(SESSION.browser_context);
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
    await client.deleteBrowserContext(BROWSER_CONTEXT.id);

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
      }),
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/session-templates'),
      expect.objectContaining({ method: 'GET' }),
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
      template_id: TEMPLATE.id,
      browser_context: {
        mode: 'reusable',
        context_id: BROWSER_CONTEXT.id,
      },
      idle_timeout_sec: 300,
      labels: { source: 'admin-smoke' },
    });

    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/sessions'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          template_id: TEMPLATE.id,
          browser_context: {
            mode: 'reusable',
            context_id: BROWSER_CONTEXT.id,
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
