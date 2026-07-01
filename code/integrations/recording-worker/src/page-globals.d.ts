export {};

type BpaneExampleUser = {
  username: string;
  password: string;
};

type BpaneAuthApi = {
  isConfigured?: () => boolean;
  isAuthenticated?: () => boolean;
  getExampleUser?: () => BpaneExampleUser | null;
};

type BpaneControlApi = {
  refreshSessions: (options?: {
    preserveSelection?: boolean;
    silent?: boolean;
  }) => Promise<unknown>;
  selectSession: (sessionId: string) => Promise<unknown>;
  connectSelected: (options?: {
    clientRole?: "interactive" | "recorder";
    gatewayUrl?: string;
  }) => Promise<unknown>;
  disconnect: () => Promise<void>;
  getState?: () => {
    connected?: boolean;
  };
};

type BpaneRecordingApi = {
  start: () => Promise<unknown>;
  stop: () => Promise<Blob>;
  downloadLast: () => void;
  setAutoDownload: (enabled: boolean) => boolean;
};

type BpaneRecorderApi = {
  connect: (options: {
    gatewayUrl: string;
    connectTicket: string;
  }) => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<{ size: number; type: string }>;
  downloadLast: () => void;
  disconnect: () => Promise<void>;
};

declare global {
  interface Window {
    __bpaneAuth?: BpaneAuthApi;
    __bpaneControl?: BpaneControlApi;
    __bpaneRecording?: BpaneRecordingApi;
    __bpaneRecorder?: BpaneRecorderApi;
  }
}
