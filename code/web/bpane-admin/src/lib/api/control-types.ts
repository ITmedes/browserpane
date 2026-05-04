export type SessionRuntimeInfo = {
  readonly binding: string;
  readonly compatibility_mode: string;
  readonly cdp_endpoint?: string | null;
};

export type SessionConnectInfo = {
  readonly gateway_url: string;
  readonly transport_path: string;
  readonly compatibility_mode: string;
};

export type SessionStopBlocker = {
  readonly kind: string;
  readonly count: number;
};

export type SessionStopEligibility = {
  readonly allowed: boolean;
  readonly blockers: readonly SessionStopBlocker[];
};

export type SessionConnectionCounts = {
  readonly interactive_clients: number;
  readonly owner_clients: number;
  readonly viewer_clients: number;
  readonly recorder_clients: number;
  readonly automation_clients: number;
  readonly total_clients: number;
};

export type SessionStatusSummary = {
  readonly runtime_state: string;
  readonly presence_state: string;
  readonly connection_counts: SessionConnectionCounts;
  readonly stop_eligibility: SessionStopEligibility;
};

export type SessionResource = {
  readonly id: string;
  readonly state: string;
  readonly owner_mode: string;
  readonly connect: SessionConnectInfo;
  readonly runtime: SessionRuntimeInfo;
  readonly status: SessionStatusSummary;
  readonly created_at: string;
  readonly updated_at: string;
  readonly stopped_at?: string | null;
};

export type SessionListResponse = {
  readonly sessions: readonly SessionResource[];
};

export type CreateSessionCommand = {
  readonly owner_mode?: string;
  readonly idle_timeout_sec?: number;
  readonly labels?: Readonly<Record<string, string>>;
};
