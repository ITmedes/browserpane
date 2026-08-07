import {
  AuthenticatedApiClient,
  formatAdminApiRequestError,
  type AccessTokenProvider,
  type FetchLike,
} from '$lib/api/authenticated-api';
import type {
  CreateWorkflowEventSubscriptionRequest,
  WorkflowEventDeliveryListResponse,
  WorkflowEventSubscriptionListResponse,
  WorkflowEventSubscriptionResource,
} from './workflow-event-types';
import { WorkflowEventCatalogError } from './workflow-event-error';
import { WorkflowEventWireMapper } from './workflow-event-wire-mapper';

export { WorkflowEventCatalogError } from './workflow-event-error';

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
      ...(options.onAuthenticationFailure === undefined
        ? {}
        : { onAuthenticationFailure: options.onAuthenticationFailure }),
      errorFactory: (failure) =>
        new WorkflowEventCatalogError(
          formatAdminApiRequestError('Workflow event subscription request', failure),
          failure.code,
          failure.status,
          failure,
        ),
    });
  }

  async listSubscriptions(): Promise<WorkflowEventSubscriptionListResponse> {
    const response = await this.#request('/api/v1/workflow-event-subscriptions', { method: 'GET' });
    return WorkflowEventWireMapper.toSubscriptionList(await response.json());
  }

  async createSubscription(
    request: CreateWorkflowEventSubscriptionRequest,
  ): Promise<WorkflowEventSubscriptionResource> {
    const response = await this.#request('/api/v1/workflow-event-subscriptions', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(request),
    });
    return WorkflowEventWireMapper.toSubscription(await response.json());
  }

  async getSubscription(subscriptionId: string): Promise<WorkflowEventSubscriptionResource> {
    const response = await this.#request(
      `/api/v1/workflow-event-subscriptions/${encodeURIComponent(subscriptionId)}`,
      { method: 'GET' },
    );
    return WorkflowEventWireMapper.toSubscription(await response.json());
  }

  async deleteSubscription(subscriptionId: string): Promise<WorkflowEventSubscriptionResource> {
    const response = await this.#request(
      `/api/v1/workflow-event-subscriptions/${encodeURIComponent(subscriptionId)}`,
      { method: 'DELETE' },
    );
    return WorkflowEventWireMapper.toSubscription(await response.json());
  }

  async listDeliveries(subscriptionId: string): Promise<WorkflowEventDeliveryListResponse> {
    const response = await this.#request(
      `/api/v1/workflow-event-subscriptions/${encodeURIComponent(subscriptionId)}/deliveries`,
      { method: 'GET' },
    );
    return WorkflowEventWireMapper.toDeliveryList(await response.json());
  }

  async #request(path: string, init: RequestInit): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    return await this.#api.request(new URL(path, this.#baseUrl), { ...init, headers });
  }
}
