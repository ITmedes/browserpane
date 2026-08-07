import {
  AdminApiRequestError,
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type AdminApiRequestErrorCode,
  type AdminApiRequestFailure,
  type FetchLike,
} from '$lib/api/authenticated-api';
import type {
  CreateWorkflowEventSubscriptionRequest,
  WorkflowEventDeliveryAttemptResource,
  WorkflowEventDeliveryListResponse,
  WorkflowEventDeliveryResource,
  WorkflowEventDeliveryState,
  WorkflowEventSubscriptionListResponse,
  WorkflowEventSubscriptionResource,
} from './workflow-event-types';

const DELIVERY_STATES = ['pending', 'delivering', 'delivered', 'failed'] satisfies readonly WorkflowEventDeliveryState[];

export class WorkflowEventCatalogError extends AdminApiRequestError {
  constructor(
    message: string,
    code: AdminApiRequestErrorCode,
    status: number | null = null,
    failure?: AdminApiRequestFailure,
  ) {
    super(message, failure ?? { code, status, message });
    this.name = 'WorkflowEventCatalogError';
  }
}

export class WorkflowEventCatalogClient {
  readonly #baseUrl: URL;
  readonly #api: AuthenticatedApiClient;

  constructor(options: {
    readonly baseUrl: string | URL;
    readonly accessTokenProvider: AccessTokenProvider;
    readonly fetchImpl?: FetchLike;
    readonly onAuthenticationFailure?: () => void;
  }) {
    this.#baseUrl = new URL(options.baseUrl);
    this.#api = new AuthenticatedApiClient({
      baseUrl: this.#baseUrl,
      accessTokenProvider: options.accessTokenProvider,
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
      ...(options.onAuthenticationFailure === undefined ? {} : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) => new WorkflowEventCatalogError(
        formatAdminApiRequestError('Workflow event subscription request', failure),
        failure.code,
        failure.status,
        failure,
      ),
    });
  }

  async listSubscriptions(): Promise<WorkflowEventSubscriptionListResponse> {
    const response = await this.#request('/api/v1/workflow-event-subscriptions', { method: 'GET' });
    return toWorkflowEventSubscriptionListResponse(await response.json());
  }

  async createSubscription(request: CreateWorkflowEventSubscriptionRequest): Promise<WorkflowEventSubscriptionResource> {
    const response = await this.#request('/api/v1/workflow-event-subscriptions', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(request),
    });
    return toWorkflowEventSubscriptionResource(await response.json());
  }

  async getSubscription(subscriptionId: string): Promise<WorkflowEventSubscriptionResource> {
    const response = await this.#request(`/api/v1/workflow-event-subscriptions/${encodeURIComponent(subscriptionId)}`, { method: 'GET' });
    return toWorkflowEventSubscriptionResource(await response.json());
  }

  async deleteSubscription(subscriptionId: string): Promise<WorkflowEventSubscriptionResource> {
    const response = await this.#request(`/api/v1/workflow-event-subscriptions/${encodeURIComponent(subscriptionId)}`, { method: 'DELETE' });
    return toWorkflowEventSubscriptionResource(await response.json());
  }

  async listDeliveries(subscriptionId: string): Promise<WorkflowEventDeliveryListResponse> {
    const response = await this.#request(
      `/api/v1/workflow-event-subscriptions/${encodeURIComponent(subscriptionId)}/deliveries`,
      { method: 'GET' },
    );
    return toWorkflowEventDeliveryListResponse(await response.json());
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    return await this.#api.request(new URL(path, this.#baseUrl), { ...init, headers });
  }
}

export function toWorkflowEventSubscriptionListResponse(payload: unknown): WorkflowEventSubscriptionListResponse {
  const object = expectRecord(payload, 'workflow event subscription list response');
  return { subscriptions: expectArray(object.subscriptions, 'workflow event subscriptions').map(toWorkflowEventSubscriptionResource) };
}

export function toWorkflowEventSubscriptionResource(value: unknown): WorkflowEventSubscriptionResource {
  const object = expectRecord(value, 'workflow event subscription');
  return {
    id: expectString(object.id, 'workflow event subscription id'),
    name: expectString(object.name, 'workflow event subscription name'),
    target_url: expectString(object.target_url, 'workflow event subscription target_url'),
    event_types: expectArray(object.event_types, 'workflow event subscription event_types').map((eventType) => expectString(eventType, 'workflow event subscription event type')),
    has_signing_secret: expectBoolean(object.has_signing_secret, 'workflow event subscription has_signing_secret'),
    deliveries_path: expectString(object.deliveries_path, 'workflow event subscription deliveries_path'),
    created_at: expectString(object.created_at, 'workflow event subscription created_at'),
    updated_at: expectString(object.updated_at, 'workflow event subscription updated_at'),
  };
}

export function toWorkflowEventDeliveryListResponse(payload: unknown): WorkflowEventDeliveryListResponse {
  const object = expectRecord(payload, 'workflow event delivery list response');
  return { deliveries: expectArray(object.deliveries, 'workflow event deliveries').map(toWorkflowEventDeliveryResource) };
}

export function toWorkflowEventDeliveryResource(value: unknown): WorkflowEventDeliveryResource {
  const object = expectRecord(value, 'workflow event delivery');
  if (object.payload === undefined || object.payload === null) {
    throw new WorkflowEventCatalogError('workflow event delivery payload must be present.', 'invalid_payload');
  }
  return {
    id: expectString(object.id, 'workflow event delivery id'),
    subscription_id: expectString(object.subscription_id, 'workflow event delivery subscription_id'),
    run_id: expectString(object.run_id, 'workflow event delivery run_id'),
    event_id: expectString(object.event_id, 'workflow event delivery event_id'),
    event_type: expectString(object.event_type, 'workflow event delivery event_type'),
    state: expectEnum(object.state, DELIVERY_STATES, 'workflow event delivery state'),
    attempt_count: expectNumber(object.attempt_count, 'workflow event delivery attempt_count'),
    next_attempt_at: optionalString(object.next_attempt_at, 'workflow event delivery next_attempt_at') ?? null,
    last_attempt_at: optionalString(object.last_attempt_at, 'workflow event delivery last_attempt_at') ?? null,
    delivered_at: optionalString(object.delivered_at, 'workflow event delivery delivered_at') ?? null,
    last_response_status: optionalNumber(object.last_response_status, 'workflow event delivery last_response_status') ?? null,
    last_error: optionalString(object.last_error, 'workflow event delivery last_error') ?? null,
    payload: object.payload,
    attempts: expectArray(object.attempts, 'workflow event delivery attempts').map(toWorkflowEventDeliveryAttemptResource),
    created_at: expectString(object.created_at, 'workflow event delivery created_at'),
    updated_at: expectString(object.updated_at, 'workflow event delivery updated_at'),
  };
}

function toWorkflowEventDeliveryAttemptResource(value: unknown): WorkflowEventDeliveryAttemptResource {
  const object = expectRecord(value, 'workflow event delivery attempt');
  return {
    id: expectString(object.id, 'workflow event delivery attempt id'),
    delivery_id: expectString(object.delivery_id, 'workflow event delivery attempt delivery_id'),
    attempt_number: expectNumber(object.attempt_number, 'workflow event delivery attempt_number'),
    response_status: optionalNumber(object.response_status, 'workflow event delivery attempt response_status') ?? null,
    error: optionalString(object.error, 'workflow event delivery attempt error') ?? null,
    created_at: expectString(object.created_at, 'workflow event delivery attempt created_at'),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> { if (!value || typeof value !== 'object' || Array.isArray(value)) throw new WorkflowEventCatalogError(`${label} must be an object.`, 'invalid_payload'); return value as Record<string, unknown>; }
function expectArray(value: unknown, label: string): readonly unknown[] { if (!Array.isArray(value)) throw new WorkflowEventCatalogError(`${label} must be an array.`, 'invalid_payload'); return value; }
function expectString(value: unknown, label: string): string { if (typeof value !== 'string') throw new WorkflowEventCatalogError(`${label} must be a string.`, 'invalid_payload'); return value; }
function optionalString(value: unknown, label: string): string | null | undefined { return value === undefined || value === null ? value : expectString(value, label); }
function expectBoolean(value: unknown, label: string): boolean { if (typeof value !== 'boolean') throw new WorkflowEventCatalogError(`${label} must be a boolean.`, 'invalid_payload'); return value; }
function expectNumber(value: unknown, label: string): number { if (typeof value !== 'number' || !Number.isFinite(value)) throw new WorkflowEventCatalogError(`${label} must be a finite number.`, 'invalid_payload'); return value; }
function optionalNumber(value: unknown, label: string): number | null | undefined { return value === undefined || value === null ? value : expectNumber(value, label); }
function expectEnum<T extends string>(value: unknown, allowed: readonly T[], label: string): T { const candidate = expectString(value, label); if (!allowed.includes(candidate as T)) throw new WorkflowEventCatalogError(`${label} must be one of ${allowed.join(', ')}.`, 'invalid_payload'); return candidate as T; }
