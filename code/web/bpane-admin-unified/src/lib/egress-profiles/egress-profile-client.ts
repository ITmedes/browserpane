import type {
  EgressCustomCaConfig,
  EgressDiagnosticsHealth,
  EgressDiagnosticsProof,
  EgressDiagnosticsProofLevel,
  EgressDiagnosticsResource,
  EgressProfileEffectiveStatus,
  EgressProfileListResponse,
  EgressProfileProjectOptionsResponse,
  EgressProfileProjectResource,
  EgressProfileResource,
  EgressProfileState,
  EgressProxyConfig,
  EgressTrafficObservationConfig,
  EgressTrafficObservationMode,
  UpsertEgressProfileRequest,
} from './egress-profile-types';

const EGRESS_PROFILE_STATES = ['ready', 'disabled'] satisfies readonly EgressProfileState[];
const EGRESS_OBSERVATION_MODES = ['metadata_only', 'tls_intercept'] satisfies readonly EgressTrafficObservationMode[];
const EGRESS_HEALTHS = ['ready', 'unknown', 'attention', 'blocked', 'missing'] satisfies readonly EgressDiagnosticsHealth[];
const EGRESS_PROOF_LEVELS = [
  'none',
  'configuration',
  'runtime_launch_metadata',
  'active_probe',
] satisfies readonly EgressDiagnosticsProofLevel[];

export type AccessTokenProvider = () => Promise<string | null> | string | null;
export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export type EgressProfileCatalogErrorCode = 'missing_token' | 'http_error' | 'invalid_payload';

export type EgressProfileCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export class EgressProfileCatalogError extends Error {
  readonly status: number | null;
  readonly code: EgressProfileCatalogErrorCode;

  constructor(message: string, code: EgressProfileCatalogErrorCode, status: number | null = null) {
    super(message);
    this.name = 'EgressProfileCatalogError';
    this.code = code;
    this.status = status;
  }
}
export class EgressProfileCatalogClient {
  readonly #baseUrl: URL;
  readonly #accessTokenProvider: AccessTokenProvider;
  readonly #fetchImpl: FetchLike;
  readonly #onAuthenticationFailure: (() => void) | undefined;

  constructor(options: EgressProfileCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#accessTokenProvider = options.accessTokenProvider;
    this.#fetchImpl = options.fetchImpl ?? fetch;
    this.#onAuthenticationFailure = options.onAuthenticationFailure;
  }

  async listEgressProfiles(): Promise<EgressProfileListResponse> {
    const response = await this.#request(new URL('/api/v1/egress-profiles', this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });

    return toEgressProfileListResponse(await response.json());
  }

  async getEgressProfile(profileId: string): Promise<EgressProfileResource> {
    const response = await this.#request(
      new URL(`/api/v1/egress-profiles/${encodeURIComponent(profileId)}`, this.#baseUrl),
      {
        method: 'GET',
        headers: {
          accept: 'application/json',
        },
      },
    );

    return toEgressProfileResource(await response.json());
  }

  async createEgressProfile(request: UpsertEgressProfileRequest): Promise<EgressProfileResource> {
    const response = await this.#request(new URL('/api/v1/egress-profiles', this.#baseUrl), {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
      },
      body: JSON.stringify(request),
    });

    return toEgressProfileResource(await response.json());
  }

  async updateEgressProfile(profileId: string, request: UpsertEgressProfileRequest): Promise<EgressProfileResource> {
    const response = await this.#request(
      new URL(`/api/v1/egress-profiles/${encodeURIComponent(profileId)}`, this.#baseUrl),
      {
        method: 'PUT',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        body: JSON.stringify(request),
      },
    );

    return toEgressProfileResource(await response.json());
  }

  async listProjectOptions(): Promise<EgressProfileProjectOptionsResponse> {
    const response = await this.#request(new URL('/api/v1/projects', this.#baseUrl), {
      method: 'GET',
      headers: {
        accept: 'application/json',
      },
    });

    return toEgressProfileProjectOptionsResponse(await response.json());
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    const accessToken = await this.#accessTokenProvider();
    if (!accessToken) {
      throw new EgressProfileCatalogError('No active admin access token is available.', 'missing_token');
    }

    const headers = new Headers(init.headers);
    headers.set('authorization', `Bearer ${accessToken}`);

    const response = await this.#fetchImpl(input, {
      ...init,
      headers,
    });
    if (response.status === 401) {
      this.#onAuthenticationFailure?.();
    }
    if (!response.ok) {
      throw new EgressProfileCatalogError(
        `Egress profile catalog request failed with HTTP ${response.status}.`,
        'http_error',
        response.status,
      );
    }

    return response;
  }
}

export function toEgressProfileListResponse(payload: unknown): EgressProfileListResponse {
  const object = expectRecord(payload, 'egress profile list response');
  const profiles = expectArray(object.profiles, 'egress profile list profiles').map(toEgressProfileResource);
  return { profiles };
}

export function toEgressProfileProjectOptionsResponse(payload: unknown): EgressProfileProjectOptionsResponse {
  const object = expectRecord(payload, 'project list response');
  const projects = expectArray(object.projects, 'project list projects').map(toEgressProjectResourceFromRequired);
  return { projects };
}

export function toEgressProfileResource(value: unknown): EgressProfileResource {
  const object = expectRecord(value, 'egress profile');
  return {
    id: expectString(object.id, 'egress profile id'),
    project_id: optionalString(object.project_id, 'egress profile project_id') ?? null,
    project: toEgressProjectResource(object.project) ?? null,
    name: expectString(object.name, 'egress profile name'),
    description: optionalString(object.description, 'egress profile description') ?? null,
    labels: toStringRecord(object.labels ?? {}, 'egress profile labels'),
    proxy: toEgressProxyConfig(object.proxy) ?? null,
    bypass_rules: toStringArray(object.bypass_rules ?? [], 'egress profile bypass_rules'),
    custom_ca: toEgressCustomCaConfig(object.custom_ca) ?? null,
    traffic_observation: toEgressTrafficObservationConfig(object.traffic_observation),
    state: expectEnum(object.state, EGRESS_PROFILE_STATES, 'egress profile state'),
    effective: toEgressEffectiveStatus(object.effective),
    diagnostics: toEgressDiagnosticsResource(object.diagnostics),
    created_at: expectString(object.created_at, 'egress profile created_at'),
    updated_at: expectString(object.updated_at, 'egress profile updated_at'),
  };
}

function toEgressProjectResource(value: unknown): EgressProfileProjectResource | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return toEgressProjectResourceFromRequired(value);
}

function toEgressProjectResourceFromRequired(value: unknown): EgressProfileProjectResource {
  const object = expectRecord(value, 'egress profile project');
  return {
    id: expectString(object.id, 'egress profile project id'),
    name: expectString(object.name, 'egress profile project name'),
    state: expectEnum(object.state, ['active', 'archived'], 'egress profile project state'),
  };
}

function toEgressProxyConfig(value: unknown): EgressProxyConfig | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'egress profile proxy');
  return {
    url: expectString(object.url, 'egress profile proxy url'),
    credential_binding_id: optionalString(
      object.credential_binding_id,
      'egress profile proxy credential_binding_id',
    ) ?? null,
  };
}

function toEgressCustomCaConfig(value: unknown): EgressCustomCaConfig | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  const object = expectRecord(value, 'egress profile custom_ca');
  return {
    certificate_ref: expectString(object.certificate_ref, 'egress profile custom_ca certificate_ref'),
    display_name: optionalString(object.display_name, 'egress profile custom_ca display_name') ?? null,
  };
}

function toEgressTrafficObservationConfig(value: unknown): EgressTrafficObservationConfig {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress profile traffic_observation');
  return {
    mode: object.mode === undefined || object.mode === null
      ? 'metadata_only'
      : expectEnum(object.mode, EGRESS_OBSERVATION_MODES, 'egress profile traffic_observation mode'),
    sensitive_log_sink_ref: optionalString(
      object.sensitive_log_sink_ref,
      'egress profile traffic_observation sensitive_log_sink_ref',
    ) ?? null,
    sensitive_log_sink_display_name: optionalString(
      object.sensitive_log_sink_display_name,
      'egress profile traffic_observation sensitive_log_sink_display_name',
    ) ?? null,
  };
}

function toEgressEffectiveStatus(value: unknown): EgressProfileEffectiveStatus {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress profile effective');
  return {
    proxy_configured: expectBoolean(object.proxy_configured ?? false, 'egress profile effective proxy_configured'),
    proxy_auth_configured: expectBoolean(
      object.proxy_auth_configured ?? false,
      'egress profile effective proxy_auth_configured',
    ),
    bypass_rule_count: expectNumber(object.bypass_rule_count ?? 0, 'egress profile effective bypass_rule_count'),
    custom_ca_configured: expectBoolean(
      object.custom_ca_configured ?? false,
      'egress profile effective custom_ca_configured',
    ),
    observation_mode: object.observation_mode === undefined || object.observation_mode === null
      ? 'metadata_only'
      : expectEnum(object.observation_mode, EGRESS_OBSERVATION_MODES, 'egress profile effective observation_mode'),
    tls_interception_enabled: expectBoolean(
      object.tls_interception_enabled ?? false,
      'egress profile effective tls_interception_enabled',
    ),
    sensitive_log_sink_configured: expectBoolean(
      object.sensitive_log_sink_configured ?? false,
      'egress profile effective sensitive_log_sink_configured',
    ),
  };
}

function toEgressDiagnosticsResource(value: unknown): EgressDiagnosticsResource {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress diagnostics');
  return {
    profile_id: optionalString(object.profile_id, 'egress diagnostics profile_id') ?? null,
    profile_name: optionalString(object.profile_name, 'egress diagnostics profile_name') ?? null,
    profile_state: object.profile_state === undefined || object.profile_state === null
      ? null
      : expectEnum(object.profile_state, EGRESS_PROFILE_STATES, 'egress diagnostics profile_state'),
    health: object.health === undefined || object.health === null
      ? 'unknown'
      : expectEnum(object.health, EGRESS_HEALTHS, 'egress diagnostics health'),
    observation_mode: object.observation_mode === undefined || object.observation_mode === null
      ? 'metadata_only'
      : expectEnum(object.observation_mode, EGRESS_OBSERVATION_MODES, 'egress diagnostics observation_mode'),
    proof_level: object.proof_level === undefined || object.proof_level === null
      ? 'none'
      : expectEnum(object.proof_level, EGRESS_PROOF_LEVELS, 'egress diagnostics proof_level'),
    runtime_binding: optionalString(object.runtime_binding, 'egress diagnostics runtime_binding') ?? null,
    runtime_assignment: optionalString(object.runtime_assignment, 'egress diagnostics runtime_assignment') ?? null,
    proxy_configured: expectBoolean(object.proxy_configured ?? false, 'egress diagnostics proxy_configured'),
    proxy_auth_configured: expectBoolean(
      object.proxy_auth_configured ?? false,
      'egress diagnostics proxy_auth_configured',
    ),
    bypass_rule_count: expectNumber(object.bypass_rule_count ?? 0, 'egress diagnostics bypass_rule_count'),
    custom_ca_configured: expectBoolean(
      object.custom_ca_configured ?? false,
      'egress diagnostics custom_ca_configured',
    ),
    tls_interception_enabled: expectBoolean(
      object.tls_interception_enabled ?? false,
      'egress diagnostics tls_interception_enabled',
    ),
    sensitive_log_sink_configured: expectBoolean(
      object.sensitive_log_sink_configured ?? false,
      'egress diagnostics sensitive_log_sink_configured',
    ),
    proof: toEgressDiagnosticsProof(object.proof),
    warnings: toStringArray(object.warnings ?? [], 'egress diagnostics warnings'),
    observed_at: optionalString(object.observed_at, 'egress diagnostics observed_at') ?? '',
  };
}

function toEgressDiagnosticsProof(value: unknown): EgressDiagnosticsProof {
  const object = value === undefined || value === null
    ? {}
    : expectRecord(value, 'egress diagnostics proof');
  return {
    profile_resolved: expectBoolean(object.profile_resolved ?? false, 'egress diagnostics proof profile_resolved'),
    profile_ready: expectBoolean(object.profile_ready ?? false, 'egress diagnostics proof profile_ready'),
    profile_reachability_collected: expectBoolean(
      object.profile_reachability_collected ?? false,
      'egress diagnostics proof profile_reachability_collected',
    ),
    profile_reachability_healthy: expectBoolean(
      object.profile_reachability_healthy ?? false,
      'egress diagnostics proof profile_reachability_healthy',
    ),
    profile_reachability_observed_at: optionalString(
      object.profile_reachability_observed_at,
      'egress diagnostics proof profile_reachability_observed_at',
    ) ?? null,
    profile_reachability_failure: optionalString(
      object.profile_reachability_failure,
      'egress diagnostics proof profile_reachability_failure',
    ) ?? null,
    proxy_launch_config_expected: expectBoolean(
      object.proxy_launch_config_expected ?? false,
      'egress diagnostics proof proxy_launch_config_expected',
    ),
    bypass_rules_expected: expectNumber(
      object.bypass_rules_expected ?? 0,
      'egress diagnostics proof bypass_rules_expected',
    ),
    custom_ca_launch_config_expected: expectBoolean(
      object.custom_ca_launch_config_expected ?? false,
      'egress diagnostics proof custom_ca_launch_config_expected',
    ),
    tls_interception_expected: expectBoolean(
      object.tls_interception_expected ?? false,
      'egress diagnostics proof tls_interception_expected',
    ),
    sensitive_log_sink_declared: expectBoolean(
      object.sensitive_log_sink_declared ?? false,
      'egress diagnostics proof sensitive_log_sink_declared',
    ),
    runtime_launch_observed: expectBoolean(
      object.runtime_launch_observed ?? false,
      'egress diagnostics proof runtime_launch_observed',
    ),
    active_probe_collected: expectBoolean(
      object.active_probe_collected ?? false,
      'egress diagnostics proof active_probe_collected',
    ),
    observed_public_ip: optionalString(object.observed_public_ip, 'egress diagnostics proof observed_public_ip') ?? null,
    observed_tls_issuer: optionalString(
      object.observed_tls_issuer,
      'egress diagnostics proof observed_tls_issuer',
    ) ?? null,
    last_failure_reason: optionalString(
      object.last_failure_reason,
      'egress diagnostics proof last_failure_reason',
    ) ?? null,
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new EgressProfileCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new EgressProfileCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string') {
    throw new EgressProfileCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | null | undefined {
  if (value === undefined || value === null) {
    return value;
  }
  return expectString(value, label);
}

function expectNumber(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new EgressProfileCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') {
    throw new EgressProfileCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}

function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T {
  const stringValue = expectString(value, label);
  if (!allowed.includes(stringValue as T)) {
    throw new EgressProfileCatalogError(
      `${label} must be one of ${allowed.join(', ')}.`,
      'invalid_payload',
    );
  }
  return stringValue as T;
}

function toStringRecord(value: unknown, label: string): Readonly<Record<string, string>> {
  const object = expectRecord(value, label);
  return Object.fromEntries(
    Object.entries(object).map(([key, item]) => [key, expectString(item, `${label}.${key}`)]),
  );
}

function toStringArray(value: unknown, label: string): readonly string[] {
  const entries = expectArray(value, label);
  return entries.map((entry, index) => expectString(entry, `${label}[${index}]`));
}
