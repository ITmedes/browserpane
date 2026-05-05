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
  readonly renderBackend?: 'auto' | 'canvas2d' | 'webgl2';
  readonly scrollCopy?: boolean;
  readonly onConnect?: () => void;
  readonly onDisconnect?: (reason: string) => void;
  readonly onError?: (error: Error) => void;
};

export type BrowserSessionHandle = {
  readonly disconnect: () => void;
  readonly getFrameCount?: () => number;
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
