import { describe, expect, it } from 'vitest';

import type { McpBridgeConfig } from '$lib/auth/auth-config';
import { sessionResource } from '$lib/test-utils/session-fixtures';
import {
  buildMcpDelegationViewModel,
  isDelegatedToBridge,
  sessionEndpointUrl,
} from './mcp-delegation-view-model';
import type { McpBridgeHealth } from './mcp-bridge-client';

const BRIDGE: McpBridgeConfig = {
  controlUrl: 'http://localhost:8080/api/v1/mcp-bridge/control-session',
  endpointBaseUrl: 'http://localhost:8931',
  clientId: 'bpane-mcp-bridge',
  issuer: 'http://localhost:8091/realms/browserpane',
  displayName: 'Local MCP bridge',
};

describe('buildMcpDelegationViewModel', () => {
  it('marks an undelegated selected session as authorizable and endpoint-copyable', () => {
    const model = buildMcpDelegationViewModel({
      bridge: BRIDGE,
      session: sessionResource({ id: 'session-1', automationDelegate: null }),
      health: health({ controlSessionId: null }),
      busy: false,
    });

    expect(model.statusLabel).toBe('Not authorized');
    expect(model.canAuthorize).toBe(true);
    expect(model.canRevoke).toBe(false);
    expect(model.canSetDefault).toBe(true);
    expect(model.endpointUrl).toBe('http://localhost:8931/sessions/session-1/mcp');
  });

  it('falls back to the control URL origin for older bridge configs', () => {
    const { endpointBaseUrl: _endpointBaseUrl, ...legacyBridge } = BRIDGE;
    expect(sessionEndpointUrl({
      ...legacyBridge,
      controlUrl: 'http://localhost:8931/control-session',
    }, 'session-1')).toBe('http://localhost:8931/sessions/session-1/mcp');
  });

  it('distinguishes authorized default sessions from active session-scoped clients', () => {
    const model = buildMcpDelegationViewModel({
      bridge: BRIDGE,
      session: sessionResource({
        id: 'session-1',
        automationDelegate: delegate(),
      }),
      health: health({
        controlSessionId: 'session-1',
        managedSessions: [{
          kind: 'control',
          session_id: 'session-1',
          clients: 2,
          state: 'ready',
          mode: 'docker_pool',
          visible: true,
          backend_delegated: true,
          mcp_owner: true,
          cdp_endpoint: null,
          playwright_cdp_endpoint: null,
          playwright_effective_cdp_endpoint: null,
          alignment: 'aligned',
        }],
      }),
      busy: false,
    });

    expect(model.statusLabel).toBe('Authorized default');
    expect(model.canAuthorize).toBe(false);
    expect(model.canClearDefault).toBe(true);
    expect(model.canRevoke).toBe(false);
    expect(model.clientSummary).toContain('2 MCP clients');
    expect(model.ownershipLabel).toBe('MCP owns this session');
  });

  it('disables actions when the bridge is not configured', () => {
    const model = buildMcpDelegationViewModel({
      bridge: null,
      session: sessionResource(),
      health: null,
      busy: false,
    });

    expect(model.statusLabel).toBe('Not configured');
    expect(model.canRefresh).toBe(false);
    expect(model.canAuthorize).toBe(false);
    expect(model.endpointUrl).toBeNull();
  });
});

describe('MCP delegation helpers', () => {
  it('matches delegates by client id and issuer and builds session endpoints', () => {
    const session = sessionResource({ automationDelegate: delegate() });

    expect(isDelegatedToBridge(session, BRIDGE)).toBe(true);
    expect(isDelegatedToBridge(session, { ...BRIDGE, issuer: 'other' })).toBe(false);
    expect(sessionEndpointUrl(BRIDGE, 'session/id')).toBe('http://localhost:8931/sessions/session%2Fid/mcp');
  });
});

function health(overrides: {
  readonly controlSessionId?: string | null;
  readonly managedSessions?: McpBridgeHealth['managed_sessions'];
} = {}): McpBridgeHealth {
  return {
    status: 'ok',
    clients: 0,
    control_session_id: overrides.controlSessionId ?? null,
    control_session_state: overrides.controlSessionId ? 'ready' : null,
    control_session_backend_delegated: Boolean(overrides.controlSessionId),
    bridge_alignment: overrides.controlSessionId ? 'aligned' : 'unmanaged',
    managed_sessions: overrides.managedSessions ?? [],
  };
}

function delegate(): Record<string, unknown> {
  return {
    client_id: BRIDGE.clientId,
    issuer: BRIDGE.issuer,
    display_name: BRIDGE.displayName,
  };
}
