import type { McpBridgeConfig } from '../auth/auth-config';
import type { McpBridgeHealth, McpManagedSessionHealth } from '../api/mcp-bridge-client';
import type { SessionResource } from '../api/control-types';

export type McpDelegationTone = 'ready' | 'active' | 'warning' | 'unavailable';

export type McpDelegationViewModel = {
  readonly title: string;
  readonly status: string;
  readonly note: string;
  readonly tone: McpDelegationTone;
  readonly canRefresh: boolean;
  readonly canAuthorize: boolean;
  readonly canRevoke: boolean;
  readonly canSetDefault: boolean;
  readonly canClearDefault: boolean;
  readonly endpointUrl: string | null;
  readonly canCopyEndpoint: boolean;
  readonly healthSummary: string | null;
  readonly busy: boolean;
  readonly error: string | null;
};

export type McpDelegationViewModelInput = {
  readonly bridge: McpBridgeConfig | null;
  readonly session: SessionResource | null;
  readonly health: McpBridgeHealth | null;
  readonly busy: boolean;
  readonly error: string | null;
};

export class McpDelegationViewModelBuilder {
  static build(input: McpDelegationViewModelInput): McpDelegationViewModel {
    const bridgeName = input.bridge?.displayName ?? input.bridge?.clientId ?? 'MCP bridge';
    const selectedId = input.session?.id ?? null;
    const controlId = input.health?.control_session_id ?? null;
    const authorized = isDelegatedToBridge(input.session, input.bridge);
    const defaultSelected = Boolean(selectedId && controlId === selectedId);
    const selectedHealth = selectedManagedSession(input.health, selectedId);
    if (!input.bridge) {
      return viewModel(bridgeName, 'Unavailable', 'MCP bridge delegation is not configured.', 'unavailable', input);
    }
    if (!selectedId) {
      return viewModel(bridgeName, 'No session selected', 'Select a session before delegating MCP.', 'ready', input);
    }
    if (input.error) {
      return viewModel(bridgeName, authorized ? 'Authorized, bridge unchecked' : 'Bridge unavailable', input.error, 'warning', input);
    }
    if (authorized && defaultSelected) {
      return viewModel(bridgeName, 'Authorized default', `${bridgeName} uses this session for legacy /mcp and session-scoped clients.`, 'active', input);
    }
    if (authorized && selectedHealth && selectedHealth.clients > 0) {
      return viewModel(bridgeName, 'Authorized active', `${bridgeName} has session-scoped MCP clients attached here.`, 'active', input);
    }
    if (authorized) {
      return viewModel(bridgeName, 'Authorized', `${shortId(selectedId)} can be reached through its session-scoped MCP endpoint.`, 'active', input);
    }
    if (controlId) {
      return viewModel(bridgeName, 'Not authorized', `${bridgeName} default is session ${shortId(controlId)}. Authorize this session to use its endpoint.`, 'warning', input);
    }
    return viewModel(bridgeName, 'Not authorized', `${shortId(selectedId)} is not authorized for ${bridgeName}.`, 'ready', input);
  }
}

function viewModel(
  title: string,
  status: string,
  note: string,
  tone: McpDelegationTone,
  input: McpDelegationViewModelInput,
): McpDelegationViewModel {
  const selectedId = input.session?.id ?? null;
  const controlId = input.health?.control_session_id ?? null;
  const authorized = isDelegatedToBridge(input.session, input.bridge);
  const defaultSelected = Boolean(selectedId && controlId === selectedId);
  const canAct = Boolean(input.bridge && selectedId) && !input.busy;
  return {
    title,
    status,
    note,
    tone,
    canRefresh: Boolean(input.bridge) && !input.busy,
    canAuthorize: canAct && !authorized,
    canRevoke: canAct && authorized && !defaultSelected,
    canSetDefault: canAct && !defaultSelected,
    canClearDefault: canAct && defaultSelected,
    endpointUrl: sessionEndpointUrl(input.bridge, input.session?.id ?? null),
    canCopyEndpoint: Boolean(sessionEndpointUrl(input.bridge, input.session?.id ?? null)) && !input.busy,
    healthSummary: managedSessionSummary(input.health, input.session?.id ?? null),
    busy: input.busy,
    error: input.error,
  };
}

function isDelegatedToBridge(session: SessionResource | null, bridge: McpBridgeConfig | null): boolean {
  const delegate = session?.automation_delegate;
  if (!delegate || !bridge) {
    return false;
  }
  return delegate.client_id === bridge.clientId && (!bridge.issuer || delegate.issuer === bridge.issuer);
}

function sessionEndpointUrl(bridge: McpBridgeConfig | null, sessionId: string | null): string | null {
  if (!bridge || !sessionId) {
    return null;
  }
  const url = new URL(bridge.controlUrl);
  url.pathname = `/sessions/${encodeURIComponent(sessionId)}/mcp`;
  url.search = '';
  url.hash = '';
  return url.toString();
}

function managedSessionSummary(health: McpBridgeHealth | null, sessionId: string | null): string | null {
  if (!health || !sessionId) {
    return null;
  }
  const managedSession = selectedManagedSession(health, sessionId);
  if (!managedSession) {
    return 'No MCP clients are attached to this session endpoint.';
  }
  const owner = managedSession.mcp_owner === null
    ? 'MCP ownership unknown'
    : managedSession.mcp_owner ? 'MCP owns session' : 'MCP does not own session';
  const alignment = managedSession.alignment ? ` · ${managedSession.alignment}` : '';
  return `${managedSession.clients} ${pluralize(managedSession.clients, 'MCP client')} · ${owner}${alignment}`;
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
