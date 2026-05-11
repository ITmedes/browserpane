import type { GatewaySessionResource } from "./session-control-client.js";

export type BridgeHealthAlignment =
  | "unmanaged"
  | "aligned"
  | "control_session_not_visible"
  | "control_session_not_delegated"
  | "playwright_endpoint_mismatch";

export type ManagedSessionHealth = {
  kind: "control" | "selected";
  session_id: string | null;
  clients: number;
  state: string | null;
  mode: string | null;
  visible: boolean;
  backend_delegated: boolean;
  mcp_owner: boolean | null;
  cdp_endpoint: string | null;
  playwright_cdp_endpoint: string | null;
  playwright_effective_cdp_endpoint: string | null;
  alignment: BridgeHealthAlignment;
};

export type ManagedSessionHealthInput = {
  kind: ManagedSessionHealth["kind"];
  session: GatewaySessionResource | null;
  visibleSession: GatewaySessionResource | null;
  clients: number;
  backendDelegated: boolean;
  mcpOwner: boolean | null;
  cdpEndpoint: string | null;
  playwrightCdpEndpoint: string | null;
  playwrightEffectiveCdpEndpoint: string | null;
  alignment: BridgeHealthAlignment;
};

export function buildManagedSessionHealth(
  input: ManagedSessionHealthInput,
): ManagedSessionHealth {
  return {
    kind: input.kind,
    session_id: input.session?.id ?? null,
    clients: input.clients,
    state: input.visibleSession?.state ?? input.session?.state ?? null,
    mode:
      input.visibleSession?.connect.compatibility_mode
      ?? input.session?.connect.compatibility_mode
      ?? null,
    visible: Boolean(input.visibleSession),
    backend_delegated: input.backendDelegated,
    mcp_owner: input.mcpOwner,
    cdp_endpoint: input.cdpEndpoint,
    playwright_cdp_endpoint: input.playwrightCdpEndpoint,
    playwright_effective_cdp_endpoint: input.playwrightEffectiveCdpEndpoint,
    alignment: input.alignment,
  };
}
