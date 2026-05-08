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
  readonly canDelegate: boolean;
  readonly canClear: boolean;
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
    const backendDelegated = this.#isDelegatedToBridge(input.session, input.bridge);
    const selectedHealth = selectedManagedSession(input.health, selectedId);
    if (!input.bridge) {
      return viewModel(bridgeName, 'Unavailable', 'MCP bridge delegation is not configured.', 'unavailable', input);
    }
    if (!selectedId) {
      return viewModel(bridgeName, 'No session selected', 'Select a session before delegating MCP.', 'ready', input);
    }
    if (input.error) {
      return viewModel(bridgeName, backendDelegated ? 'Delegated, bridge unchecked' : 'Bridge unavailable', input.error, 'warning', input, true);
    }
    if (selectedHealth && selectedHealth.clients > 0 && controlId !== selectedId) {
      return viewModel(bridgeName, 'Session endpoint active', `${bridgeName} has direct MCP clients on this session endpoint.`, 'active', input, true);
    }
    if (!controlId && backendDelegated) {
      return viewModel(bridgeName, 'Backend delegated', `${bridgeName} has backend access but no attached control session.`, 'warning', input, true);
    }
    if (!controlId) {
      return viewModel(bridgeName, 'No delegated session', `${shortId(selectedId)} is not attached to ${bridgeName}.`, 'ready', input, true);
    }
    if (controlId === selectedId) {
      return viewModel(bridgeName, 'This session delegated', `${bridgeName} drives the selected browser session.`, 'active', input, true, true);
    }
    return viewModel(bridgeName, `Session ${shortId(controlId)} delegated`, `${bridgeName} is attached to a different session.`, 'warning', input, true, true);
  }

  static #isDelegatedToBridge(session: SessionResource | null, bridge: McpBridgeConfig | null): boolean {
    const delegate = session?.automation_delegate;
    if (!delegate || !bridge) {
      return false;
    }
    return delegate.client_id === bridge.clientId && (!bridge.issuer || delegate.issuer === bridge.issuer);
  }
}

function viewModel(
  title: string,
  status: string,
  note: string,
  tone: McpDelegationTone,
  input: McpDelegationViewModelInput,
  canDelegate = false,
  canClear = false,
): McpDelegationViewModel {
  return {
    title,
    status,
    note,
    tone,
    canRefresh: Boolean(input.bridge) && !input.busy,
    canDelegate: canDelegate && !input.busy,
    canClear: canClear && !input.busy,
    endpointUrl: sessionEndpointUrl(input.bridge, input.session?.id ?? null),
    canCopyEndpoint: Boolean(sessionEndpointUrl(input.bridge, input.session?.id ?? null)) && !input.busy,
    healthSummary: managedSessionSummary(input.health, input.session?.id ?? null),
    busy: input.busy,
    error: input.error,
  };
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
