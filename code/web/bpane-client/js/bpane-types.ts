export type RenderBackendPreference = 'auto' | 'canvas2d' | 'webgl2';

export interface BpaneOptions {
  container: HTMLElement;
  gatewayUrl: string;
  /** OIDC/JWT access token for the gateway session. */
  accessToken?: string;
  /** Legacy dev-token compatibility path. Prefer `accessToken`. */
  token?: string;
  hiDpi?: boolean;
  audio?: boolean;
  microphone?: boolean;
  camera?: boolean;
  clipboard?: boolean;
  fileTransfer?: boolean;
  /** URL to fetch TLS cert SHA-256 hash (base64) for self-signed certs */
  certHashUrl?: string;
  /** Diagnostic backend override. Defaults to 'auto'. */
  renderBackend?: RenderBackendPreference;
  /** Diagnostic switch. Disable retained scroll-copy reuse and rely on repair tiles only. */
  scrollCopy?: boolean;
  onConnect?: () => void;
  onDisconnect?: (reason: string) => void;
  onError?: (error: Error) => void;
  onCapabilitiesChange?: (capabilities: SessionCapabilities) => void;
  onResolutionChange?: (width: number, height: number) => void;
}

export interface SessionCapabilities {
  audio: boolean;
  microphone: boolean;
  camera: boolean;
  fileTransfer: boolean;
  keyboardLayout: boolean;
}
