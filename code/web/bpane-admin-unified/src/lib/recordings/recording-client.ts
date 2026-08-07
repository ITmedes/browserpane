import {
  AdminApiRequestError,
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type AdminApiRequestErrorCode,
  type AdminApiRequestFailure,
  type FetchLike,
} from '$lib/api/authenticated-api';
import type { SessionResource } from '$lib/sessions/session-types';
import type {
  RecordingCatalogEntry,
  RecordingCatalogLoadFailure,
  SessionRecordingListResponse,
  SessionRecordingResource,
} from './recording-types';

export type { AccessTokenProvider, FetchLike } from '$lib/api/authenticated-api';
export type RecordingCatalogErrorCode = AdminApiRequestErrorCode;

export type RecordingCatalogClientOptions = {
  readonly baseUrl: string | URL;
  readonly accessTokenProvider: AccessTokenProvider;
  readonly fetchImpl?: FetchLike;
  readonly onAuthenticationFailure?: () => void;
};

export type RecordingCatalogResponse = {
  readonly entries: readonly RecordingCatalogEntry[];
  readonly failures: readonly RecordingCatalogLoadFailure[];
};

export class RecordingCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: RecordingCatalogErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'RecordingCatalogError';
  }
}

export class RecordingCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: RecordingCatalogClientOptions) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new RecordingCatalogError(
        formatAdminApiRequestError('Recording catalog request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listRecordingsForSessions(sessions: readonly SessionResource[]): Promise<RecordingCatalogResponse> {
    const results = await Promise.all(sessions.map(async (session) => {
      try {
        const response = await this.listSessionRecordings(session.id);
        return {
          session,
          recordings: response.recordings,
          failure: null,
        };
      } catch (error) {
        return {
          session,
          recordings: [],
          failure: {
            sessionId: session.id,
            message: error instanceof Error ? error.message : 'Session recordings could not be loaded.',
          },
        };
      }
    }));

    return {
      entries: results.flatMap((result) =>
        result.recordings.map((recording) => ({
          session: result.session,
          recording,
        })),
      ),
      failures: results
        .map((result) => result.failure)
        .filter((failure): failure is RecordingCatalogLoadFailure => failure !== null),
    };
  }

  async listSessionRecordings(sessionId: string): Promise<SessionRecordingListResponse> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/recordings`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/json' },
      },
    );
    return toSessionRecordingListResponse(await response.json());
  }

  async downloadRecordingContent(recording: SessionRecordingResource): Promise<Blob> {
    const response = await this.#request(new URL(recording.content_path, this.#baseUrl), {
      method: 'GET',
      headers: { accept: recording.mime_type ?? '*/*' },
    });
    return await response.blob();
  }

  async downloadSessionPlaybackExport(sessionId: string): Promise<Blob> {
    const response = await this.#request(
      new URL(`/api/v1/sessions/${encodeURIComponent(sessionId)}/recording-playback/export`, this.#baseUrl),
      {
        method: 'GET',
        headers: { accept: 'application/zip, application/octet-stream' },
      },
    );
    return await response.blob();
  }

  async #request(input: URL, init: RequestInit): Promise<Response> {
    return await this.#api.request(input, init);
  }
}

export function toSessionRecordingListResponse(payload: unknown): SessionRecordingListResponse {
  const object = expectRecord(payload, 'session recording list response');
  return {
    recordings: expectArray(object.recordings, 'session recording list recordings').map(toSessionRecordingResource),
  };
}

export function toSessionRecordingResource(payload: unknown): SessionRecordingResource {
  const object = expectRecord(payload, 'session recording');
  return {
    id: expectString(object.id, 'session recording id'),
    session_id: expectString(object.session_id, 'session recording session_id'),
    previous_recording_id: optionalString(object.previous_recording_id, 'session recording previous_recording_id') ?? null,
    state: expectString(object.state, 'session recording state'),
    format: expectString(object.format, 'session recording format'),
    mime_type: optionalString(object.mime_type, 'session recording mime_type') ?? null,
    bytes: optionalNumber(object.bytes, 'session recording bytes') ?? null,
    duration_ms: optionalNumber(object.duration_ms, 'session recording duration_ms') ?? null,
    error: optionalString(object.error, 'session recording error') ?? null,
    termination_reason: optionalString(object.termination_reason, 'session recording termination_reason') ?? null,
    artifact_available: optionalBoolean(object.artifact_available, 'session recording artifact_available') ?? false,
    content_path: expectString(object.content_path, 'session recording content_path'),
    started_at: expectString(object.started_at, 'session recording started_at'),
    completed_at: optionalString(object.completed_at, 'session recording completed_at') ?? null,
    created_at: expectString(object.created_at, 'session recording created_at'),
    updated_at: expectString(object.updated_at, 'session recording updated_at'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new RecordingCatalogError(`${label} must be an object.`, 'invalid_payload');
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new RecordingCatalogError(`${label} must be an array.`, 'invalid_payload');
  }
  return value;
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new RecordingCatalogError(`${label} must be a non-empty string.`, 'invalid_payload');
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'string') {
    throw new RecordingCatalogError(`${label} must be a string.`, 'invalid_payload');
  }
  return value;
}

function optionalNumber(value: unknown, label: string): number | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new RecordingCatalogError(`${label} must be a finite number.`, 'invalid_payload');
  }
  return value;
}

function optionalBoolean(value: unknown, label: string): boolean | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== 'boolean') {
    throw new RecordingCatalogError(`${label} must be a boolean.`, 'invalid_payload');
  }
  return value;
}
