export type BrowserSessionRenderBackend = 'auto' | 'canvas2d' | 'webgl2';

export type BrowserSessionConnectPreferences = {
  readonly hiDpi: boolean;
  readonly audio: boolean;
  readonly microphone: boolean;
  readonly camera: boolean;
  readonly clipboard: boolean;
  readonly fileTransfer: boolean;
  readonly renderBackend: BrowserSessionRenderBackend;
  readonly scrollCopy: boolean;
};

export const DEFAULT_BROWSER_SESSION_CONNECT_PREFERENCES = Object.freeze({
  hiDpi: true,
  audio: true,
  microphone: true,
  camera: true,
  clipboard: true,
  fileTransfer: true,
  renderBackend: 'auto',
  scrollCopy: true,
} satisfies BrowserSessionConnectPreferences);

export type BrowserSessionConnectOptions = {
  readonly container: HTMLElement;
  readonly gatewayUrl: string;
  readonly connectTicket?: string;
  readonly accessToken?: string;
  readonly clientRole?: 'interactive' | 'recorder';
  readonly hiDpi?: boolean;
  readonly audio?: boolean;
  readonly microphone?: boolean;
  readonly camera?: boolean;
  readonly clipboard?: boolean;
  readonly fileTransfer?: boolean;
  readonly certHashUrl?: string;
  readonly renderBackend?: BrowserSessionRenderBackend;
  readonly scrollCopy?: boolean;
  readonly onConnect?: () => void;
  readonly onDisconnect?: (reason: string) => void;
  readonly onError?: (error: Error) => void;
};

export type BrowserSessionHandle = {
  readonly disconnect: () => void;
  readonly getFrameCount?: () => number;
  readonly startMicrophone?: () => Promise<void>;
  readonly stopMicrophone?: () => void;
  readonly startCamera?: () => Promise<void>;
  readonly stopCamera?: () => void;
  readonly uploadFiles?: (files: FileList | Iterable<File>) => Promise<void>;
};

export type BrowserSessionSdk = {
  readonly BpaneSession: {
    readonly connect: (options: BrowserSessionConnectOptions) => Promise<BrowserSessionHandle>;
  };
};

export type LiveBrowserSessionConnection = {
  readonly sessionId: string;
  readonly gatewayUrl: string;
  readonly handle: BrowserSessionHandle;
};
