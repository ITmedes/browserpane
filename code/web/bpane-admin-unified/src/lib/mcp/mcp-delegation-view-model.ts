import type { McpBridgeConfig } from '$lib/auth/auth-config';
import type { SessionResource } from '$lib/sessions/session-types';
import type { McpBridgeHealth, McpManagedSessionHealth } from './mcp-bridge-client';

export type McpDelegationTone = 'neutral' | 'success' | 'warning' | 'danger';

export type McpDelegationViewModel = {
  readonly title: string;
  readonly statusLabel: string;
  readonly statusTone: McpDelegationTone;
  readonly description: string;
  readonly endpointUrl: string | null;
  readonly defaultSessionLabel: string;
  readonly clientSummary: string;
  readonly ownershipLabel: string;
  readonly bridgeSummary: string;
  readonly canRefresh: boolean;
  readonly canAuthorize: boolean;
  readonly canRevoke: boolean;
  readonly canSetDefault: boolean;
  readonly canClearDefault: boolean;
  readonly canCopyEndpoint: boolean;
};

export type McpDelegationViewModelInput = {
  readonly bridge: McpBridgeConfig | null;
  readonly session: SessionResource | null;
  readonly health: McpBridgeHealth | null;
  readonly busy: boolean;
};

export function buildMcpDelegationViewModel(input: McpDelegationViewModelInput): McpDelegationViewModel {
  const bridgeName = bridgeDisplayName(input.bridge);
  const sessionId = input.session?.id ?? null;
  const authorized = isDelegatedToBridge(input.session, input.bridge);
  const defaultSelected = Boolean(sessionId && input.health?.control_session_id === sessionId);
  const selectedHealth = selectedManagedSession(input.health, sessionId);
  const canAct = Boolean(input.bridge && sessionId) && !input.busy;

  if (!input.bridge) {
    return model({
      title: 'MCP delegation',
      statusLabel: 'Not configured',
      statusTone: 'danger',
      description: 'No MCP bridge metadata is available for this admin deployment.',
      endpointUrl: null,
      defaultSessionLabel: 'Unavailable',
      clientSummary: 'No bridge health available.',
      ownershipLabel: 'Unknown',
      bridgeSummary: 'Configure auth-config mcpBridge metadata to enable delegation controls.',
      canRefresh: false,
      canAuthorize: false,
      canRevoke: false,
      canSetDefault: false,
      canClearDefault: false,
      canCopyEndpoint: false,
    });
  }

  if (!sessionId) {
    return model({
      title: bridgeName,
      statusLabel: 'No session',
      statusTone: 'neutral',
      description: 'Select a session before authorizing an MCP bridge.',
      endpointUrl: null,
      defaultSessionLabel: defaultSessionLabel(input.health),
      clientSummary: bridgeClientSummary(input.health),
      ownershipLabel: 'Inactive',
      bridgeSummary: bridgeHealthSummary(input.health),
      canRefresh: !input.busy,
      canAuthorize: false,
      canRevoke: false,
      canSetDefault: false,
      canClearDefault: false,
      canCopyEndpoint: false,
    });
  }

  const endpointUrl = sessionEndpointUrl(input.bridge, sessionId);
  const activeClients = selectedHealth ? selectedHealth.clients > 0 : false;
  const status = delegationStatus({
    authorized,
    defaultSelected,
    activeClients,
    hasDefault: Boolean(input.health?.control_session_id),
  });

  return model({
    title: bridgeName,
    statusLabel: status.label,
    statusTone: status.tone,
    description: status.description(bridgeName, sessionId, input.health?.control_session_id ?? null),
    endpointUrl,
    defaultSessionLabel: defaultSelected ? 'This session is the bridge default' : defaultSessionLabel(input.health),
    clientSummary: selectedHealth ? managedSessionSummary(selectedHealth) : 'No MCP clients are attached to this session endpoint.',
    ownershipLabel: selectedHealth?.mcp_owner ? 'MCP owns this session' : 'MCP ownership inactive',
    bridgeSummary: bridgeHealthSummary(input.health),
    canRefresh: !input.busy,
    canAuthorize: canAct && !authorized,
    canRevoke: canAct && authorized && !defaultSelected,
    canSetDefault: canAct && !defaultSelected,
    canClearDefault: canAct && defaultSelected,
    canCopyEndpoint: Boolean(endpointUrl) && !input.busy,
  });
}

export function isDelegatedToBridge(session: SessionResource | null, bridge: McpBridgeConfig | null): boolean {
  const delegate = session?.automation_delegate;
  if (!delegate || !bridge) {
    return false;
  }
  return delegate.client_id === bridge.clientId && (!bridge.issuer || delegate.issuer === bridge.issuer);
}

export function sessionEndpointUrl(bridge: McpBridgeConfig | null, sessionId: string | null): string | null {
  if (!bridge || !sessionId) {
    return null;
  }
  const url = new URL(bridge.controlUrl);
  url.pathname = `/sessions/${encodeURIComponent(sessionId)}/mcp`;
  url.search = '';
  url.hash = '';
  return url.toString();
}

function model(value: McpDelegationViewModel): McpDelegationViewModel {
  return value;
}

function bridgeDisplayName(bridge: McpBridgeConfig | null): string {
  return bridge?.displayName || bridge?.clientId || 'MCP bridge';
}

function delegationStatus(input: {
  readonly authorized: boolean;
  readonly defaultSelected: boolean;
  readonly activeClients: boolean;
  readonly hasDefault: boolean;
}): {
  readonly label: string;
  readonly tone: McpDelegationTone;
  readonly description: (bridgeName: string, sessionId: string, defaultSessionId: string | null) => string;
} {
  if (input.authorized && input.defaultSelected) {
    return {
      label: 'Authorized default',
      tone: 'success',
      description: (bridgeName) =>
        `${bridgeName} can use this session through its session endpoint and through the compatibility /mcp endpoint.`,
    };
  }
  if (input.authorized && input.activeClients) {
    return {
      label: 'Authorized active',
      tone: 'success',
      description: (bridgeName) => `${bridgeName} has session-scoped MCP clients attached to this session.`,
    };
  }
  if (input.authorized) {
    return {
      label: 'Authorized',
      tone: 'success',
      description: (bridgeName) => `${bridgeName} can connect to this session through the session-scoped MCP endpoint.`,
    };
  }
  if (input.hasDefault) {
    return {
      label: 'Not authorized',
      tone: 'warning',
      description: (_bridgeName, sessionId, defaultSessionId) =>
        `${shortId(sessionId)} is not authorized. The bridge default currently points to ${shortId(defaultSessionId ?? 'unknown')}.`,
    };
  }
  return {
    label: 'Not authorized',
    tone: 'neutral',
    description: (bridgeName, sessionId) => `${shortId(sessionId)} is not authorized for ${bridgeName}.`,
  };
}

function defaultSessionLabel(health: McpBridgeHealth | null): string {
  if (!health) {
    return 'Bridge status not loaded';
  }
  if (!health.control_session_id) {
    return 'No default session selected';
  }
  const delegated = health.control_session_backend_delegated ? 'delegated' : 'not delegated';
  return `${shortId(health.control_session_id)} (${delegated})`;
}

function bridgeClientSummary(health: McpBridgeHealth | null): string {
  if (!health) {
    return 'No bridge health available.';
  }
  return `${health.clients} ${pluralize(health.clients, 'bridge client')}`;
}

function bridgeHealthSummary(health: McpBridgeHealth | null): string {
  if (!health) {
    return 'Bridge health has not been refreshed yet.';
  }
  const alignment = health.bridge_alignment ? ` | ${health.bridge_alignment}` : '';
  return `${health.status}${alignment}`;
}

function managedSessionSummary(session: McpManagedSessionHealth): string {
  const visibility = session.visible ? 'visible' : 'not visible';
  const delegated = session.backend_delegated ? 'delegated' : 'not delegated';
  const alignment = session.alignment ? ` | ${session.alignment}` : '';
  return `${session.clients} ${pluralize(session.clients, 'MCP client')} | ${visibility} | ${delegated}${alignment}`;
}

function selectedManagedSession(
  health: McpBridgeHealth | null,
  sessionId: string | null,
): McpManagedSessionHealth | null {
  if (!health || !sessionId) {
    return null;
  }
  return health.managed_sessions.find((entry) => entry.session_id === sessionId) ?? null;
}

function pluralize(count: number, singular: string): string {
  return count === 1 ? singular : `${singular}s`;
}

function shortId(value: string): string {
  return value.length > 13 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}
