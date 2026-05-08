import { describe, expect, it } from 'vitest';
import type { McpBridgeConfig } from '../auth/auth-config';
import type { McpBridgeHealth } from '../api/mcp-bridge-client';
import type { SessionResource } from '../api/control-types';
import { McpDelegationViewModelBuilder } from './mcp-delegation-view-model';

const BRIDGE: McpBridgeConfig = {
  controlUrl: 'http://localhost:8931/control-session',
  clientId: 'bpane-mcp-bridge',
  issuer: 'http://localhost:8091/realms/bpane',
  displayName: 'BrowserPane MCP bridge',
};

describe('McpDelegationViewModelBuilder', () => {
  it('enables delegation when a configured bridge and selected session exist', () => {
    const viewModel = McpDelegationViewModelBuilder.build({
      bridge: BRIDGE,
      session: sessionResource('session-a'),
      health: null,
      busy: false,
      error: null,
    });

    expect(viewModel.status).toBe('No delegated session');
    expect(viewModel.canDelegate).toBe(true);
    expect(viewModel.canClear).toBe(false);
    expect(viewModel.endpointUrl).toBe('http://localhost:8931/sessions/session-a/mcp');
    expect(viewModel.canCopyEndpoint).toBe(true);
  });

  it('marks the selected session as active when bridge control matches', () => {
    const viewModel = McpDelegationViewModelBuilder.build({
      bridge: BRIDGE,
      session: sessionResource('session-a'),
      health: health('session-a'),
      busy: false,
      error: null,
    });

    expect(viewModel.status).toBe('This session delegated');
    expect(viewModel.tone).toBe('active');
    expect(viewModel.canClear).toBe(true);
  });

  it('surfaces direct session endpoint health for the selected session', () => {
    const viewModel = McpDelegationViewModelBuilder.build({
      bridge: BRIDGE,
      session: sessionResource('session-a'),
      health: health(null, [{
        kind: 'selected',
        session_id: 'session-a',
        clients: 1,
        state: 'active',
        mode: 'session_runtime_pool',
        visible: true,
        backend_delegated: true,
        mcp_owner: true,
        cdp_endpoint: 'http://runtime:9223',
        playwright_cdp_endpoint: 'http://runtime:9223',
        playwright_effective_cdp_endpoint: 'http://runtime:9223',
        alignment: 'aligned',
      }]),
      busy: false,
      error: null,
    });

    expect(viewModel.status).toBe('Session endpoint active');
    expect(viewModel.healthSummary).toBe('1 MCP client · MCP owns session · aligned');
    expect(viewModel.canClear).toBe(false);
  });

  it('surfaces stale backend delegation when bridge health is unavailable', () => {
    const viewModel = McpDelegationViewModelBuilder.build({
      bridge: BRIDGE,
      session: sessionResource('session-a', true),
      health: null,
      busy: false,
      error: 'Bridge status could not be loaded.',
    });

    expect(viewModel.status).toBe('Delegated, bridge unchecked');
    expect(viewModel.tone).toBe('warning');
    expect(viewModel.canDelegate).toBe(true);
  });
});

function health(
  sessionId: string | null,
  managedSessions: McpBridgeHealth['managed_sessions'] = [],
): McpBridgeHealth {
  return {
    status: 'ok',
    clients: 0,
    control_session_id: sessionId,
    control_session_state: sessionId ? 'active' : null,
    control_session_backend_delegated: Boolean(sessionId),
    bridge_alignment: 'aligned',
    managed_sessions: managedSessions,
  };
}

function sessionResource(id: string, delegated = false): SessionResource {
  return {
    id,
    state: 'active',
    owner_mode: 'shared',
    automation_delegate: delegated ? {
      client_id: BRIDGE.clientId,
      issuer: BRIDGE.issuer,
      display_name: BRIDGE.displayName,
    } : null,
    connect: {
      gateway_url: 'https://localhost:4433',
      transport_path: '/session',
      auth_type: 'session_connect_ticket',
      compatibility_mode: 'session_runtime_pool',
    },
    runtime: { binding: 'docker_runtime_pool', compatibility_mode: 'session_runtime_pool' },
    status: {
      runtime_state: 'running',
      presence_state: 'connected',
      connection_counts: {
        interactive_clients: 0,
        owner_clients: 0,
        viewer_clients: 0,
        recorder_clients: 0,
        automation_clients: 0,
        total_clients: 0,
      },
      stop_eligibility: { allowed: true, blockers: [] },
    },
    created_at: '2026-05-04T19:00:00Z',
    updated_at: '2026-05-04T19:01:00Z',
    stopped_at: null,
  };
}
